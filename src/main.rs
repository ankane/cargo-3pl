use serde_json::Value;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use structopt::clap::AppSettings;
use structopt::StructOpt;

struct Package {
    name: String,
    version: String,
    url: Option<String>,
    license: Option<String>,
    path: PathBuf,
    license_paths: Vec<PathBuf>,
}

enum Color {
    Red = 31,
    Yellow = 33,
}

// try to match output of other cargo commands
#[derive(Debug, StructOpt)]
#[structopt(name = "cargo-3pl", about, usage = "cargo 3pl [OPTIONS]", global_settings = &[AppSettings::UnifiedHelpMessage])]
struct Opt {
    /// Space or comma separated list of features to activate
    #[structopt(long, value_name = "FEATURES")]
    features: Vec<String>,

    /// Activate all available features
    #[structopt(long)]
    all_features: bool,

    /// Do not activate the `default` feature
    #[structopt(long)]
    no_default_features: bool,

    // cargo passes 3pl
    // this approach allows cargo-3pl 3pl but that's fine
    #[structopt(hidden = true, possible_values = &["3pl"])]
    _cmd: Option<String>,
}

fn license_filename(filename: &str) -> bool {
    filename.contains("license") || filename.contains("licence") || filename.contains("notice") || filename.contains("copying")
}

fn license_ext(ext: &str) -> bool {
    ext.is_empty() || ext == "txt" || ext == "md"
}

fn license_file(path: &Path) -> bool {
    let filename = path.file_stem().unwrap_or_else(|| OsStr::new("")).to_string_lossy().to_lowercase();
    let ext = path.extension().unwrap_or_else(|| OsStr::new("")).to_string_lossy().to_lowercase();
    license_filename(&filename) && license_ext(&ext)
}

fn find_license_files(license_paths: &mut Vec<PathBuf>, dir: &Path) {
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                find_license_files(license_paths, &path);
            } else {
                let path = entry.path();
                if license_file(&path) {
                    license_paths.push(path);
                }
            }
        }
    }
}

// TODO use atty to detect tty
fn colorize(message: String, color: Color) -> String {
    format!("\x1b[{}m{}\x1b[0m", color as u8, message)
}

fn warn(message: String) {
    eprintln!("{}", colorize(message, Color::Yellow));
}

fn get_metadata(opt: &Opt) -> Result<Value, Box<dyn Error>> {
    let mut cmd = Command::new("cargo");
    cmd.arg("metadata");
    cmd.arg("--format-version");
    cmd.arg("1");
    for feature in &opt.features {
        cmd.arg("--features");
        cmd.arg(feature);
    }
    if opt.all_features {
        cmd.arg("--all-features");
    }
    if opt.no_default_features {
        cmd.arg("--no-default-features");
    }
    let output = cmd.output()?;

    if !output.status.success() {
        return Err("cargo metadata failed".into());
    }

    Ok(serde_json::from_slice(&output.stdout)?)
}

fn find_packages(opt: &Opt) -> Result<Vec<Package>, Box<dyn Error>> {
    let metadata = get_metadata(opt)?;
    let workspace_root = metadata["workspace_root"].as_str().unwrap();

    let mut packages = Vec::new();
    for package in metadata["packages"].as_array().unwrap() {
        let manifest_path = PathBuf::from(package["manifest_path"].as_str().unwrap());

        // there doesn't appear to be a great way to determine current package
        // https://github.com/rust-lang/cargo/issues/4018
        if manifest_path.starts_with(&workspace_root) {
            continue;
        }

        let mut license_paths = Vec::new();
        let path = manifest_path.parent().unwrap().to_path_buf();
        find_license_files(&mut license_paths, &path);
        license_paths.sort_unstable();

        packages.push(Package {
            name: package["name"].as_str().unwrap().into(),
            version: package["version"].as_str().unwrap().into(),
            url: package["homepage"].as_str().or_else(|| package["repository"].as_str()).map(|v| v.into()),
            license: package["license"].as_str().map(|v| v.into()),
            path,
            license_paths,
        })
    }

    Ok(packages)
}

fn print_header(header: String) {
    println!("{}\n{}\n{}\n", "=".repeat(80), header, "=".repeat(80));
}

fn print_packages(packages: &[Package]) -> Result<(), Box<dyn Error>> {
    print_header("Summary".into());
    for package in packages {
        println!("{} v{}", package.name, package.version);
        if let Some(url) = &package.url {
            println!("{}", url);
        }
        if let Some(license) = &package.license {
            println!("{}", license);
        }
        println!();
    }

    let mut stdout = io::stdout();
    for package in packages {
        for path in &package.license_paths {
            let mut file = File::open(&path)?;
            let relative_path = path.strip_prefix(&package.path).unwrap().display();
            print_header(format!("{} {}", package.name, relative_path));
            io::copy(&mut file, &mut stdout)?;
            println!();
        }
    }

    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_args();
    let packages = find_packages(&opt)?;

    if packages.is_empty() {
        return Err("No dependencies".into());
    }

    for package in &packages {
        if package.license.is_none() {
            warn(format!("No license field: {}", package.name));
        }
    }

    for package in &packages {
        if package.license_paths.is_empty() {
            warn(format!("No license files found: {}", package.name));
        }
    }

    print_packages(&packages)
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", colorize(err.to_string(), Color::Red));
        process::exit(1);
    }
}
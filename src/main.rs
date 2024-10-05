use clap::builder::PossibleValuesParser;
use clap::Parser;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};

struct LicenseFile {
    path: PathBuf,
    relative_path: String,
}

impl LicenseFile {
    fn new(path: PathBuf, root: &Path) -> Self {
        let relative_path = path.strip_prefix(root).unwrap().display().to_string();
        Self {
            path,
            relative_path,
        }
    }
}

struct Package {
    name: String,
    version: String,
    url: Option<String>,
    license: Option<String>,
    license_files: Vec<LicenseFile>,
    multiple_versions: bool,
}

impl Package {
    fn display_name(&self) -> String {
        if self.multiple_versions {
            self.full_name()
        } else {
            self.name.clone()
        }
    }

    fn full_name(&self) -> String {
        format!("{} v{}", self.name, self.version)
    }
}

enum Color {
    Red = 31,
    Yellow = 33,
}

// try to match output of other cargo commands
#[derive(Debug, Parser)]
#[command(
    name = "cargo-3pl",
    about,
    override_usage = "cargo 3pl [OPTIONS]",
    version
)]
struct Opt {
    /// Space or comma separated list of features to activate
    #[arg(long, value_name = "FEATURES")]
    features: Vec<String>,

    /// Activate all available features
    #[arg(long)]
    all_features: bool,

    /// Do not activate the `default` feature
    #[arg(long)]
    no_default_features: bool,

    /// Filter dependencies matching the given target-triple
    #[arg(long, value_name = "TRIPLE")]
    target: Vec<String>,

    /// Require all dependencies to have license files
    #[arg(long)]
    require_files: bool,

    /// Path for license files (experimental)
    #[arg(long, value_name = "PATH")]
    source: Option<PathBuf>,

    /// Show the package url (experimental)
    #[arg(hide = true, long)]
    show_url: bool,

    // cargo passes 3pl
    // this approach allows cargo-3pl 3pl but that's fine
    #[arg(hide = true, value_parser = PossibleValuesParser::new(&["3pl"]))]
    _cmd: Option<String>,
}

fn license_filename(filename: &str) -> bool {
    filename.contains("license")
        || filename.contains("licence")
        || filename.contains("notice")
        || filename.contains("copying")
}

fn license_ext(ext: &str) -> bool {
    ext.is_empty() || ext == "txt" || ext == "md"
}

fn license_file(path: &Path) -> bool {
    let filename = path
        .file_stem()
        .unwrap_or_else(|| OsStr::new(""))
        .to_string_lossy()
        .to_lowercase();
    let ext = path
        .extension()
        .unwrap_or_else(|| OsStr::new(""))
        .to_string_lossy()
        .to_lowercase();
    license_filename(&filename) && license_ext(&ext)
}

fn find_license_files(license_files: &mut Vec<LicenseFile>, dir: &Path, root: &Path, all: bool) {
    if dir.is_dir() {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                find_license_files(license_files, &path, root, all);
            } else {
                let path = entry.path();
                if all || license_file(&path) {
                    license_files.push(LicenseFile::new(path, root));
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
    for target in opt.target.iter() {
        cmd.arg("--filter-platform");
        cmd.arg(target);
    }
    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let spec_error = "Error loading target specification: ";
        if let Some(line) = stderr.lines().find(|v| v.contains(spec_error)) {
            return Err(line.split(spec_error).last().unwrap().into());
        } else {
            return Err(format!("cargo metadata failed\n{}", stderr).into());
        }
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
        if manifest_path.starts_with(workspace_root) {
            continue;
        }

        let name = package["name"].as_str().unwrap().into();
        let version = package["version"].as_str().unwrap().into();

        let mut license_files = Vec::new();
        let path = manifest_path.parent().unwrap().to_path_buf();
        find_license_files(&mut license_files, &path, &path, false);
        if let Some(license_file) = package["license_file"].as_str() {
            let license_path = path.join(license_file);
            if !license_files.iter().any(|v| v.path == license_path) {
                license_files.push(LicenseFile::new(license_path, &path));
            }
        }
        license_files.sort_unstable_by_key(|v| v.path.clone());
        if let Some(source) = &opt.source {
            let s = source.join(format!("{}-{}", name, version));
            find_license_files(&mut license_files, &s, &s, true);
        }

        packages.push(Package {
            name,
            version,
            url: package["homepage"]
                .as_str()
                .or_else(|| package["repository"].as_str())
                .map(|v| v.into()),
            license: package["license"].as_str().map(|v| v.into()),
            license_files,
            multiple_versions: false,
        })
    }

    let mut counts = HashMap::new();
    for package in &packages {
        *counts.entry(package.name.clone()).or_insert(0) += 1;
    }

    for package in &mut packages {
        package.multiple_versions = counts.get(&package.name).unwrap() > &1;
    }

    Ok(packages)
}

fn print_header(header: String) {
    println!("{}\n{}\n{}", "=".repeat(80), header, "=".repeat(80));
}

fn print_packages(packages: &[Package]) -> Result<(), Box<dyn Error>> {
    print_header("Summary".into());
    for package in packages {
        println!();
        println!("{} v{}", package.name, package.version);
        if let Some(url) = &package.url {
            println!("{}", url);
        }
        if let Some(license) = &package.license {
            println!("{}", license);
        }
    }

    let mut stdout = io::stdout();
    for package in packages {
        for license_file in &package.license_files {
            let mut file = File::open(&license_file.path)?;
            println!();
            print_header(format!(
                "{} {}",
                package.display_name(),
                license_file.relative_path
            ));
            println!();
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            stdout.write_all(&buffer)?;

            // ensure consistent spacing between licenses
            if let Some(v) = buffer.last() {
                if v != &10 {
                    println!();
                }
            }
        }
    }

    Ok(())
}

fn run() -> Result<(), Box<dyn Error>> {
    let opt = Opt::parse();
    let packages = find_packages(&opt)?;

    if packages.is_empty() {
        return Err("No dependencies".into());
    }

    for package in &packages {
        if package.license.is_none() {
            warn(format!("No license field: {}", package.full_name()));
        }
    }

    let mut missing_files = false;
    for package in &packages {
        if package.license_files.is_empty() {
            let mut suffix = "".into();
            if opt.show_url {
                if let Some(url) = &package.url {
                    suffix = format!(" ({})", url);
                }
            };
            warn(format!(
                "No license files found: {}{}",
                package.full_name(),
                suffix
            ));
            missing_files = true;
        }
    }
    if opt.require_files && missing_files {
        return Err("Exiting due to missing license files".into());
    }

    print_packages(&packages)
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{}", colorize(err.to_string(), Color::Red));
        process::exit(1);
    }
}

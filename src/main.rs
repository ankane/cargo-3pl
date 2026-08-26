#![doc = include_str!("../README.md")]

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

#[derive(Debug, Default)]
struct Opt {
    features: Vec<String>,
    all_features: bool,
    no_default_features: bool,
    target: Vec<String>,
    require_files: bool,
    source: Option<PathBuf>,
    show_url: bool,
}

fn license_filename(filename: &str) -> bool {
    filename.contains("license")
        || filename.contains("licence")
        || filename.contains("notice")
        || filename.contains("copying")
}

fn license_dir(path: &Path) -> bool {
    // REUSE spec
    path.iter().any(|v| v == "LICENSES")
}

fn license_ext(ext: &str) -> bool {
    ext.is_empty() || ext == "txt" || ext == "md" || ext == "lesser"
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
    (license_filename(&filename) || license_dir(path)) && license_ext(&ext)
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
fn colorize(message: &str, color: Color) -> String {
    format!("\x1b[{}m{}\x1b[0m", color as u8, message)
}

fn warn(message: &str) {
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
    for target in &opt.target {
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
            return Err(format!("cargo metadata failed\n{stderr}").into());
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
            let s = source.join(format!("{name}-{version}"));
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
        });
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

fn print_header(header: &str) {
    println!("{}\n{}\n{}", "=".repeat(80), header, "=".repeat(80));
}

fn print_packages(packages: &[Package]) -> Result<(), Box<dyn Error>> {
    print_header("Summary");
    for package in packages {
        println!();
        println!("{} v{}", package.name, package.version);
        if let Some(url) = &package.url {
            println!("{url}");
        }
        if let Some(license) = &package.license {
            println!("{license}");
        }
    }

    let mut stdout = io::stdout();
    for package in packages {
        for license_file in &package.license_files {
            let mut file = File::open(&license_file.path)?;
            println!();
            print_header(&format!(
                "{} {}",
                package.display_name(),
                license_file.relative_path
            ));
            println!();
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            stdout.write_all(&buffer)?;

            // ensure consistent spacing between licenses
            if let Some(v) = buffer.last()
                && v != &10
            {
                println!();
            }
        }
    }

    Ok(())
}

fn parse_opt_error(error: &str) {
    eprintln!("error: {}\n\nFor more information, try '--help'.", error);
    process::exit(1);
}

fn parse_opt() -> Opt {
    let mut args = std::env::args().skip(1).peekable();
    let mut opt = Opt::default();

    // cargo passes 3pl
    if let Some(arg) = args.peek() {
        if arg.as_str() == "3pl" {
            args.next();
        }
    }

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--features" => {
                if let Some(feature) = args.next() {
                    opt.features.push(feature);
                } else {
                    parse_opt_error(
                        "a value is required for '--features <FEATURES>' but none was supplied",
                    );
                }
            }
            "--all-features" => {
                opt.all_features = true;
            }
            "--no-default-features" => {
                opt.no_default_features = true;
            }
            "--target" => {
                if let Some(target) = args.next() {
                    opt.target.push(target);
                } else {
                    parse_opt_error(
                        "a value is required for '--target <TRIPLE>' but none was supplied",
                    );
                }
            }
            "--require-files" => {
                opt.require_files = true;
            }
            "--source" => {
                if let Some(source) = args.next() {
                    if !opt.source.is_none() {
                        parse_opt_error(
                            "the argument '--source <PATH>' cannot be used multiple times",
                        );
                    }
                    opt.source = Some(source.into());
                } else {
                    parse_opt_error(
                        "a value is required for '--source <PATH>' but none was supplied",
                    );
                }
            }
            "--show-url" => {
                opt.show_url = true;
            }
            "-V" | "--version" => {
                println!("cargo-3pl {}", env!("CARGO_PKG_VERSION"));
                process::exit(0);
            }
            "-h" | "--help" => {
                println!(
                    "The easy way to ship dependency licenses with your Rust binaries

Usage: cargo 3pl [OPTIONS]

Options:
      --features <FEATURES>  Space or comma separated list of features to activate
      --all-features         Activate all available features
      --no-default-features  Do not activate the `default` feature
      --target <TRIPLE>      Filter dependencies matching the given target-triple
      --require-files        Require all dependencies to have license files
      --source <PATH>        Path for license files (experimental)
  -h, --help                 Print help
  -V, --version              Print version"
                );
                process::exit(0);
            }
            _ => {
                parse_opt_error(format!("unexpected argument '{}' found", arg).as_str());
            }
        }
    }

    opt
}

fn run() -> Result<(), Box<dyn Error>> {
    let opt = parse_opt();
    let packages = find_packages(&opt)?;

    if packages.is_empty() {
        return Err("No dependencies".into());
    }

    for package in &packages {
        if package.license.is_none() {
            warn(&format!("No license field: {}", package.full_name()));
        }
    }

    let mut missing_files = false;
    for package in &packages {
        if package.license_files.is_empty() {
            let mut suffix = "".into();
            if opt.show_url
                && let Some(url) = &package.url
            {
                suffix = format!(" ({url})");
            };
            warn(&format!(
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
        eprintln!("{}", colorize(&err.to_string(), Color::Red));
        process::exit(1);
    }
}

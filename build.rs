use std::{env, fs, path::PathBuf, process::Command};
fn run(command: &mut Command) {
    let status = command.status().expect("required UI build tool unavailable");
    assert!(status.success(), "UI resource build failed: {command:?}");
}
fn main() {
    println!("cargo:rerun-if-changed=src/gtk");
    println!("cargo:rerun-if-changed=src/icons");
    println!("cargo:rerun-if-changed=src/overskride.gresource.xml");
    println!("cargo:rerun-if-env-changed=BLUEPRINT_COMPILER");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let locale=out.join("locale/de/LC_MESSAGES"); fs::create_dir_all(&locale).unwrap();
    run(Command::new("msgfmt").arg("po/de.po").arg("-o").arg(locale.join("windoesnot-bluetooth.mo")));
    println!("cargo:rerun-if-changed=po/de.po");
    println!("cargo:rustc-env=WDN_BUILT_LOCALE={}",out.join("locale").display());
    let files: Vec<_> = fs::read_dir("src/gtk").unwrap().map(|x|x.unwrap().path())
        .filter(|p|p.extension().is_some_and(|e|e=="blp")).collect();
    let compiler=env::var("BLUEPRINT_COMPILER").unwrap_or_else(|_|"blueprint-compiler".into());
    run(Command::new(compiler).arg("batch-compile").arg(&out).arg("src").args(files));
    run(Command::new("glib-compile-resources").arg("src/overskride.gresource.xml")
        .arg(format!("--sourcedir={}",out.display())).arg("--sourcedir=src")
        .arg(format!("--target={}",out.join("overskride.gresource").display())));
}

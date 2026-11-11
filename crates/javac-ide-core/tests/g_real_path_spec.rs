//! G: real Maven path at the library boundary.
//!
//! Complements `scripts/lsp/probe_g_real_path.py`. A three-file fixture
//! (App/Helper/User) exercises cross-file definition, references, rename,
//! import definition and the save -> index refresh.

mod common;
use common::platform_dir;

use std::path::{Path, PathBuf};

use javac_analysis_api::{Position, WorkspaceService};

use javac_ide_core::project::{self, LoadContext};
use javac_ide_core::workspace::Workspace;

const POM: &str = "<project><modelVersion>4.0.0</modelVersion><groupId>com.example</groupId><artifactId>real</artifactId><version>1.0</version></project>\n";
const APP: &str = r#"package com.example.app;

import com.example.base.Helper;
import com.example.other.User;

import java.util.List;

public class App {
    Helper helper = new Helper();
    User user = new User(helper);
    List<String> names;

    void f() {
        helper.compute();
        int v = helper.value;
        user.see(helper);
    }
}
"#;
const HELPER: &str = r#"package com.example.base;

public class Helper {
    public int value;

    public void compute() {
    }
}
"#;
const USER: &str = r#"package com.example.other;

import com.example.base.Helper;

public class User {
    Helper helper;

    public User(Helper helper) {
        this.helper = helper;
    }

    public void see(Helper helper) {
        int v = helper.value;
    }
}
"#;
const APP_PATH: &str = "src/main/java/com/example/app/App.java";
const HELPER_PATH: &str = "src/main/java/com/example/base/Helper.java";
const USER_PATH: &str = "src/main/java/com/example/other/User.java";

fn fixture(name: &str) -> (PathBuf, Workspace) {
    platform_dir();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("tmp")
        .join("g-real-path-spec")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("pom.xml"), POM).unwrap();
    for (relative, text) in [(APP_PATH, APP), (HELPER_PATH, HELPER), (USER_PATH, USER)] {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
    }
    let providers: Vec<Box<dyn javac_project_api::BuildSystemProvider>> =
        vec![Box::new(project::DirScanProvider)];
    let model = project::load_with(&providers, &root, &LoadContext::default());
    let mut workspace = Workspace::new_from_project(&model, root.join("out"), None);
    workspace.wait_for_index();
    (root, workspace)
}

fn position_of(text: &str, marker: &str, offset: usize) -> Position {
    let start = text.find(marker).expect("marker in fixture") + offset;
    let prefix = &text[..start];
    let line = prefix.matches('\n').count() as u32;
    let column = start - (prefix.rfind('\n').map(|index| index + 1).unwrap_or(0));
    Position::new(line, column as u32)
}

#[test]
fn cross_file_definition_references_and_rename() {
    let (root, mut workspace) = fixture("cross-file");
    let helper = root.join(HELPER_PATH);
    let app = root.join(APP_PATH);

    // definition: App -> Helper (type) and -> User.
    let d1 = workspace.definition(&app, position_of(APP, "new Helper", "new ".len()));
    assert!(
        d1.as_ref().is_some_and(|location| location.file.ends_with("Helper.java")),
        "{d1:?}"
    );
    let d2 = workspace.definition(&app, position_of(APP, "new User", "new ".len()));
    assert!(
        d2.as_ref().is_some_and(|location| location.file.ends_with("User.java")),
        "{d2:?}"
    );

    // references on the `value` field: App + User + declaration.
    let references = workspace.references(&helper, position_of(HELPER, "int value", "int ".len()));
    let files: Vec<String> = references
        .iter()
        .map(|location| location.file.to_string_lossy().to_string())
        .collect();
    assert!(
        files.iter().any(|path| path.ends_with("App.java"))
            && files.iter().any(|path| path.ends_with("User.java"))
            && files.iter().any(|path| path.ends_with("Helper.java")),
        "references: {files:?}"
    );

    // rename the class: App + User + Helper files must change.
    let edits = workspace.rename_workspace(
        &helper,
        position_of(HELPER, "class Helper", "class ".len()),
        "Helper2",
    );
    let rename_files: Vec<String> = edits
        .iter()
        .map(|edit| edit.file.to_string_lossy().to_string())
        .collect();
    assert!(
        rename_files.iter().any(|path| path.ends_with("App.java"))
            && rename_files.iter().any(|path| path.ends_with("User.java"))
            && rename_files.iter().any(|path| path.ends_with("Helper.java")),
        "rename: {rename_files:?}"
    );
}

#[test]
fn import_lines_resolve_to_project_classes() {
    let (root, mut workspace) = fixture("imports");
    let app = root.join(APP_PATH);
    // `import com.example.base.Helper;`
    let project_import = workspace.definition(&app, position_of(APP, "import com.example.base.Helper", "import com.example.base.".len()));
    assert!(
        project_import
            .as_ref()
            .is_some_and(|location| location.file.ends_with("Helper.java")),
        "project import: {project_import:?}"
    );
    // `import java.util.List;`
    let jdk_import = workspace.definition(&app, position_of(APP, "import java.util.List", "import java.util.".len()));
    assert!(
        jdk_import.as_ref().is_some_and(|location| location.file.ends_with("List.java")),
        "jdk import: {jdk_import:?}"
    );
    // `List<String> names;`
    let jdk_type = workspace.definition(&app, position_of(APP, "List<String>", 0));
    assert!(
        jdk_type.as_ref().is_some_and(|location| location.file.ends_with("List.java")),
        "jdk type: {jdk_type:?}"
    );
}

#[test]
fn save_refreshes_the_index() {
    let (root, mut workspace) = fixture("save");
    let app = root.join(APP_PATH);
    let with_method = APP.replace(
        "    void f() {",
        "    void addedMethod() {\n    }\n\n    void f() {",
    );
    std::fs::write(&app, with_method).unwrap();
    workspace.invalidate(&[app.clone()]);
    workspace.wait_for_index();
    let symbols = workspace.workspace_symbols("addedMethod");
    assert!(
        symbols.iter().any(|symbol| symbol.name == "addedMethod"),
        "{:?}",
        symbols.iter().map(|symbol| &symbol.name).collect::<Vec<_>>()
    );
}

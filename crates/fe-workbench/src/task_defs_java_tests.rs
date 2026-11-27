//! Gradle and Maven detection over a fake [`ProjectFiles`].

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use crate::task_defs::{Group, ProjectFiles, Source, Task, TaskCommand, load};
#[cfg(unix)]
use crate::test_runner::runnable;

const ROOT: &str = "/project";

/// The files there, and those among them without an execute bit.
struct Files {
    present: BTreeSet<PathBuf>,
    not_executable: BTreeSet<PathBuf>,
}

impl Files {
    fn with(names: &[&str]) -> Self {
        Self {
            present: names
                .iter()
                .map(|name| Path::new(ROOT).join(name))
                .collect(),
            not_executable: BTreeSet::new(),
        }
    }

    fn without_execute_bit(mut self, name: &str) -> Self {
        let path = Path::new(ROOT).join(name);
        assert!(self.present.contains(&path), "no {name}");
        self.not_executable.insert(path);
        self
    }

    fn tasks(&self) -> Vec<Task> {
        load(self, Path::new(ROOT)).0
    }
}

impl ProjectFiles for Files {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        if self.present.contains(path) {
            Ok(String::new())
        } else {
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }

    fn exists(&self, path: &Path) -> bool {
        self.present.contains(path)
    }

    fn is_executable(&self, path: &Path) -> bool {
        self.exists(path) && !self.not_executable.contains(path)
    }
}

fn from(tasks: &[Task], source: Source) -> Vec<&Task> {
    tasks.iter().filter(|task| task.source == source).collect()
}

fn argv(task: &Task) -> Vec<&str> {
    match &task.command {
        TaskCommand::Argv(words) => words.iter().map(String::as_str).collect(),
        other => panic!("not an argument list: {other:?}"),
    }
}

#[test]
fn gradle_root_detects_wrapper_tasks() {
    let tasks = Files::with(&["build.gradle.kts", "gradlew"]).tasks();
    let gradle = from(&tasks, Source::Gradle);
    let got: Vec<_> = gradle.iter().map(|task| (argv(task), task.group)).collect();
    assert_eq!(
        got,
        vec![
            (vec!["./gradlew", "build"], Some(Group::Build)),
            (vec!["./gradlew", "test"], Some(Group::Test)),
            (vec!["./gradlew", "run"], None),
            (vec!["./gradlew", "clean"], None),
            (vec!["./gradlew", "check"], None),
        ]
    );
    let settings_only = Files::with(&["settings.gradle"]).tasks();
    assert_eq!(from(&settings_only, Source::Gradle).len(), 5);
}

#[test]
fn gradle_without_wrapper_uses_gradle() {
    let tasks = Files::with(&["build.gradle"]).tasks();
    let gradle = from(&tasks, Source::Gradle);
    assert_eq!(argv(gradle[0]), ["gradle", "build"]);
    assert!(gradle.iter().all(|task| argv(task)[0] == "gradle"));
}

#[test]
fn gradle_with_a_non_executable_wrapper_uses_gradle() {
    let tasks = Files::with(&["build.gradle", "gradlew"])
        .without_execute_bit("gradlew")
        .tasks();
    let got: Vec<_> = from(&tasks, Source::Gradle)
        .iter()
        .map(|task| argv(task).join(" "))
        .collect();
    assert_eq!(
        got,
        [
            "gradle build",
            "gradle test",
            "gradle run",
            "gradle clean",
            "gradle check",
        ]
    );
}

#[test]
fn maven_with_a_non_executable_wrapper_uses_mvn() {
    let tasks = Files::with(&["pom.xml", "mvnw"])
        .without_execute_bit("mvnw")
        .tasks();
    let got: Vec<_> = from(&tasks, Source::Maven)
        .iter()
        .map(|task| argv(task).join(" "))
        .collect();
    assert_eq!(
        got,
        [
            "mvn compile",
            "mvn test",
            "mvn package",
            "mvn verify",
            "mvn clean",
        ]
    );
}

#[cfg(unix)]
#[test]
fn only_a_file_with_an_execute_bit_is_runnable_on_disk() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = tempfile::tempdir().expect("a temporary folder");
    let wrapper = dir.path().join("gradlew");
    assert!(!runnable(&wrapper));
    std::fs::write(&wrapper, "#!/bin/sh\n").expect("a wrapper");
    let mode = |mode| {
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(mode))
            .expect("its mode");
    };
    mode(0o644);
    assert!(!runnable(&wrapper));
    mode(0o755);
    assert!(runnable(&wrapper));
    // A folder has execute bits and still isn't a program.
    assert!(!runnable(dir.path()));
}

#[test]
fn maven_root_detects_mvn_tasks_and_prefers_mvnw() {
    let plain = Files::with(&["pom.xml"]).tasks();
    let got: Vec<_> = from(&plain, Source::Maven)
        .iter()
        .map(|task| (argv(task).join(" "), task.group))
        .collect();
    assert_eq!(
        got,
        vec![
            ("mvn compile".to_owned(), Some(Group::Build)),
            ("mvn test".to_owned(), Some(Group::Test)),
            ("mvn package".to_owned(), Some(Group::Build)),
            ("mvn verify".to_owned(), None),
            ("mvn clean".to_owned(), None),
        ]
    );
    let wrapped = Files::with(&["pom.xml", "mvnw"]).tasks();
    assert!(
        from(&wrapped, Source::Maven)
            .iter()
            .all(|task| argv(task)[0] == "./mvnw")
    );
}

#[test]
fn gradle_beats_maven_for_default_build() {
    let tasks = Files::with(&["build.gradle", "pom.xml"]).tasks();
    let defaults = |group| -> Vec<String> {
        tasks
            .iter()
            .filter(|task| task.default && task.group == Some(group))
            .map(|task| task.label.clone())
            .collect()
    };
    assert_eq!(defaults(Group::Build), ["gradle build"]);
    assert_eq!(defaults(Group::Test), ["gradle test"]);
    let maven_only = Files::with(&["pom.xml"]).tasks();
    let first = maven_only
        .iter()
        .find(|task| task.default && task.group == Some(Group::Build))
        .expect("a default build");
    assert_eq!(first.label, "mvn compile");
}

#[test]
fn detected_java_tasks_never_reach_a_shell() {
    let tasks = Files::with(&["build.gradle", "gradlew", "pom.xml", "mvnw"]).tasks();
    let java: Vec<_> = tasks
        .iter()
        .filter(|task| matches!(task.source, Source::Gradle | Source::Maven))
        .collect();
    assert_eq!(java.len(), 10);
    for task in java {
        assert!(matches!(task.command, TaskCommand::Argv(_)), "{task:?}");
    }
}

#[test]
fn a_project_with_none_gets_none() {
    let tasks = Files::with(&["README.md", "src"]).tasks();
    assert!(
        tasks
            .iter()
            .all(|task| !matches!(task.source, Source::Gradle | Source::Maven))
    );
}

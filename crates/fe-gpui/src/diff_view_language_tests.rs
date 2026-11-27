//! The language each side of a diff is highlighted in.

use super::*;

fn language(view: &Entity<EditorView>, cx: &mut VisualTestContext) -> Option<&'static str> {
    view.read_with(cx, |view, _| view.syntax().map(|s| s.language().name()))
}

/// A diff named `name` of two path-less sides.
fn pathless<'a>(
    name: &'static str,
    cx: &'a mut TestAppContext,
) -> (Entity<DiffView>, &'a mut VisualTestContext) {
    cx.update(init);
    let (diff, cx) = cx.add_window_view(move |_, cx| {
        let left = EditorModel::new(Buffer::new(""), None);
        let right = EditorModel::new(Buffer::new(""), None);
        DiffView::new(
            name,
            DiffSide::new("HEAD", left).read_only(),
            DiffSide::new("Index", right).read_only(),
            cx,
        )
    });
    draw(cx);
    (diff, cx)
}

#[gpui::test]
fn the_left_side_takes_the_files_language(app: &mut TestAppContext) {
    let right = EditorModel::new(Buffer::new("fn b() {}\n"), Some("a.rs".into()));
    let (diff, cx) = diff_view_of("fn a() {}\n", right, app);
    cx.run_until_parked();
    for ix in 0..2 {
        let view = side(&diff, ix, cx);
        assert_eq!(language(&view, cx), Some("rust"), "side {ix}");
    }
}

#[gpui::test]
fn both_sides_of_a_commit_diff_take_the_files_language(app: &mut TestAppContext) {
    for (name, expected) in [("a.rs", "rust"), ("regatta.toml", "toml")] {
        let (diff, cx) = pathless(name, app);
        cx.run_until_parked();
        for ix in 0..2 {
            let view = side(&diff, ix, cx);
            assert_eq!(language(&view, cx), Some(expected), "{name} side {ix}");
        }
    }
}

#[gpui::test]
fn a_left_side_whose_text_lands_later_keeps_the_language(app: &mut TestAppContext) {
    let (diff, cx) = pathless("a.rs", app);
    cx.run_until_parked();
    let left = side(&diff, 0, cx);
    left.update(cx, |view, cx| {
        let len = view.model().text().len();
        let mut buffer = view.model_mut().buffer_mut();
        buffer.edit([(0..len, "fn a() {}\n")], fe_buffer::Origin::User);
        drop(buffer);
        cx.notify();
    });
    draw(cx);
    cx.run_until_parked();
    let (name, parsed, current) = left.read_with(cx, |view, _| {
        let syntax = view.syntax().expect("highlighted");
        (
            syntax.language().name(),
            syntax.version(),
            view.model().snapshot().version(),
        )
    });
    assert_eq!(name, "rust");
    assert_eq!(parsed, current);
}

#[gpui::test]
fn a_side_with_a_path_keeps_its_own_language(app: &mut TestAppContext) {
    let right = EditorModel::new(Buffer::new("x = 1\n"), Some("b.py".into()));
    let (diff, cx) = diff_view_of("fn a() {}\n", right, app);
    cx.run_until_parked();
    let view = side(&diff, 1, cx);
    assert_eq!(language(&view, cx), Some("python"));
}

/// A diff of two path-less sides of the file at `compared`, named by its
/// last component, with the host's `files.associations` saying
/// `associate`.
fn compared<'a>(
    compared: &'static str,
    associate: fn(&Path) -> Option<&'static str>,
    cx: &'a mut TestAppContext,
) -> (Entity<DiffView>, &'a mut VisualTestContext) {
    cx.update(|cx| {
        init(cx);
        EditorView::set_language_associations(cx, move |path, _| associate(path));
    });
    let (diff, cx) = cx.add_window_view(move |_, cx| {
        let left = EditorModel::new(Buffer::new(""), None);
        let right = EditorModel::new(Buffer::new(""), None);
        let name = Path::new(compared).file_name().unwrap().to_string_lossy();
        DiffView::new(
            name.into_owned(),
            DiffSide::new("HEAD", left).read_only(),
            DiffSide::new("Index", right).read_only(),
            cx,
        )
        .with_compared_path(PathBuf::from(compared))
    });
    draw(cx);
    (diff, cx)
}

/// The host's associations see the whole path of the file compared, so a
/// pattern on its folders applies to a commit's diff of it, and still does
/// when the sides are detected again.
#[gpui::test]
fn path_less_sides_go_by_the_associations_of_the_file_compared(app: &mut TestAppContext) {
    let associate = |path: &Path| path.ends_with("conf/a.rs").then_some("toml");
    let (diff, cx) = compared("/work/conf/a.rs", associate, app);
    cx.run_until_parked();
    for ix in 0..2 {
        let view = side(&diff, ix, cx);
        assert_eq!(language(&view, cx), Some("toml"), "side {ix}");
    }
    diff.update(cx, |diff, cx| diff.detect_languages_again(cx));
    cx.run_until_parked();
    for ix in 0..2 {
        let view = side(&diff, ix, cx);
        assert_eq!(language(&view, cx), Some("toml"), "side {ix} again");
    }
}

/// Detected again with associations that don't match it, a diff keeps the
/// language its file's name gives it.
#[gpui::test]
fn an_association_for_other_files_leaves_a_diffs_language(app: &mut TestAppContext) {
    let associate = |path: &Path| {
        let zz = path.extension().is_some_and(|extension| extension == "zz");
        zz.then_some("yaml")
    };
    let (diff, cx) = compared("/work/src/lib.rs", associate, app);
    cx.run_until_parked();
    diff.update(cx, |diff, cx| diff.detect_languages_again(cx));
    cx.run_until_parked();
    for ix in 0..2 {
        let view = side(&diff, ix, cx);
        assert_eq!(language(&view, cx), Some("rust"), "side {ix}");
    }
}

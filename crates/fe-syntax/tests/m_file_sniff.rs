//! A `.m` file, which Objective-C (`objc`) and MATLAB (`matlab`) both claim,
//! goes to the one its start looks like when both are registered, and to
//! `objc` when it can't tell: `detect_language` sniffs it after the path. A
//! modeline still wins, the sniff never picks a language that isn't loaded
//! nor drops a file the path gave one to, and no other extension, nor a
//! third claimant of `m`, is affected. The JSON fixture stands in for both
//! grammars, as the real MATLAB one comes later (T2135).

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use fe_syntax::{GrammarSpec, detect_language, register_language, unregister_language};

const OBJC: &[&str] = &[
    "#import <Foundation/Foundation.h>\n\n@interface Foo : NSObject\n@end\n",
    "//\n//  Foo.m\n//  App\n//\n\n#import \"Foo.h\"\n",
    "/*\n * Copyright 2024\n */\n\n@implementation Foo\n- (void)bar {}\n@end\n",
    "/* one line */ #include <stdio.h>\n",
    "@import UIKit;\n",
    "#define MAX 3\n",
    "int main(int argc, char *argv[]) {\n    @autoreleasepool {\n        return 0;\n    }\n}\n",
    "@protocol Drawing\n- (void)draw;\n@end\n",
    "@class Foo;\n@property (nonatomic) int x;\n@synthesize x;\n",
    // A directive anywhere beats MATLAB's markers.
    "% looks like MATLAB\nfunction f\n#import \"Foo.h\"\n",
    // A C comment before any MATLAB marker.
    "// Foo.m\n% MATLAB?\n",
    "x = 1;\n/* C */\n% then MATLAB?\n",
    "\u{feff}#import \"Foo.h\"\n",
    // A MATLAB-looking line in a C comment opened after code.
    "int n; /*\nfunction example\n% no\n*/\n@end\n",
    "int r = a % b; /*\nfunction f\nclc\n*/\n",
    "NSString *s = @\"/* not a comment\";\n#import \"Foo.h\"\n",
    // A `%{` comment nested in another doesn't end it at its `%}`.
    "%{\n%{\n%}\n#import \"Foo.h\"\n%}\n@interface Example\n@end\n",
];

const MATLAB: &[&str] = &[
    "function y = square(x)\n% SQUARE squares x.\n  y = x.^2;\nend\n",
    "function [a, b] = swap(b, a)\nend\n",
    "function plotAll\n",
    "function out=f(x)\n",
    "function = f(x)\n",
    "function=f\n",
    "function[y] = f(x)\ny = x;\n",
    "if(x > 0)\nx = 1;\nend\n",
    "while(k < 3)\n  k = k + 1;\nend\n",
    "switch(n)\n  case 1\n    y = 1;\nend\n",
    "classdef Point < handle\n  properties\n    X\n  end\nend\n",
    "% A script.\nx = 1;\n",
    "  %% Section\n",
    "clc; clear all; close all;\n",
    "clear\nx = 1;\n",
    "clear x y\n",
    "close all\n",
    "format long\nx = pi\n",
    "x = 3;\nif x > 2\n  y = 1;\nend\n",
    "for k = 1:3\n  k\nend;\n",
    "fprintf('%d\\n', 3);\n",
    "fprintf(\"%d\\n\", 3);\n",
    "fprintf(fid, '%s\\n', name);\n",
    "disp(\"100% complete\");\n",
    "disp(sprintf(\"%d\", 3))\n",
    "disp(\"Progress: \"\"100%\"\"\");\n",
    "fprintf(\"Value: \"\"%d\"\"\", 3);\n",
    "disp('It''s 100%');\n",
    "fprintf(fid, 'x: %d\\n', x); % '%'\n",
    "fprintf(fid, '%%');\n",
    // MATLAB strings that start with an escape, or are one `%` for disp.
    "fprintf('\\n%d\\n', 3);\n",
    "fprintf(fid, '\\n%d', x);\n",
    "disp('%');\n",
    "fprintf('%');\n",
    "disp(sprintf('%5.2f', pi))\n",
    // Text in a block comment doesn't count.
    "%{\n#import \"Foo.h\"\n@end\n%}\nx = 1;\n",
    "%{\n%{\n#import \"Foo.h\"\n%}\n@end\n%}\nx = 1;\n",
    // A `/*` in a MATLAB comment or string opens no C comment.
    "x = 1; % see /* this\nclc\n",
    "x = 1 % see /* this\nclc\n",
    "y = x % a /* b */ c /* d\nclc\n",
    "s = 'a /* b';\nclc\n",
    // A C-looking comment after a MATLAB marker doesn't count.
    "% MATLAB\n// a stray line\n",
];

/// Neither language's markers: `objc`, as the path alone gives it.
const UNSURE: &[&str] = &[
    "",
    "\n\n",
    "x = 1;\ny = 2;\n",
    "foo(bar);\n",
    "fprintf(stderr, \"%d\\n\", x);\n",
    "end\n",
    "functional = 1;\nclearAll();\nclear(x);\nclc = 2;\nformat = 1;\n",
    "classdefs\n#importer\n@ending\n",
    // C, not a MATLAB declaration.
    "function(x);\nfunctions = 1;\n",
    "fprintf(stderr, \"%d\\n\", x);\ndisp(x, \"%d\");\n",
    // C character literals and comments after the call aren't formats.
    "fprintf(stderr, \"%c\", '%');\n",
    "fprintf(stderr, \"%c\", '\\%');\n",
    "fprintf(stderr, \"%d\", n); // '100%'\n",
    "fprintf(stderr, \"%d\", n); /* '100%' */\n",
    "if(x) {\n  y = 1;\n}\n",
    // A C comment closed on its line hides nothing after it.
    "int n; /* note */ %d\n",
];

/// One test at a time: they register and withdraw the same names.
fn lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Registers stand-ins for `languages`, each with its extensions, in order,
/// after withdrawing any registered by an earlier test.
fn registered(languages: &[(&str, &[&str])]) {
    for name in ["objc", "matlab", "mercury"] {
        unregister_language(name);
    }
    // Read at run time: a path compiled in can name a deleted snapshot.
    let manifest = std::env::var_os("CARGO_MANIFEST_DIR").expect("run by cargo");
    let wasm = PathBuf::from(manifest).join("tests/fixtures/tree-sitter-json.wasm");
    for (language, extensions) in languages {
        register_language(&GrammarSpec {
            language: (*language).into(),
            wasm: wasm.clone(),
            extensions: extensions.iter().map(|&e| e.into()).collect(),
            ..GrammarSpec::default()
        })
        .unwrap();
    }
}

fn detect(path: &str, text: &str) -> Option<&'static str> {
    detect_language(Some(Path::new(path)), text, "")
}

fn assert_all(path: &str, texts: &[&str], want: &str) {
    for text in texts {
        assert_eq!(detect(path, text), Some(want), "{path} {text:?}");
    }
}

#[test]
fn the_content_picks_objc_or_matlab_and_objc_when_unsure() {
    let _lock = lock();
    // Whichever registered first, so whichever the path alone gives.
    for order in [["objc", "matlab"], ["matlab", "objc"]] {
        registered(&[(order[0], &["m"]), (order[1], &["m"])]);
        for path in ["a.m", "src/Foo.M"] {
            assert_all(path, OBJC, "objc");
            assert_all(path, MATLAB, "matlab");
            assert_all(path, UNSURE, "objc");
        }
    }
}

#[test]
fn only_the_first_few_hundred_bytes_are_read() {
    let _lock = lock();
    registered(&[("objc", &["m"]), ("matlab", &["m"])]);
    let late = format!("{}% MATLAB\n", "x = 1;\n".repeat(100));
    assert_eq!(detect("a.m", &late), Some("objc"));
    let late = format!("% MATLAB\n{}#import \"Foo.h\"\n", "x = 1;\n".repeat(100));
    assert_eq!(detect("a.m", &late), Some("matlab"));
}

#[test]
fn a_modeline_beats_the_sniff() {
    let _lock = lock();
    registered(&[("objc", &["m"]), ("matlab", &["m"])]);
    let text = "#import \"Foo.h\"\n// vim: set ft=matlab:\n";
    assert_eq!(detect("a.m", text), Some("matlab"));
    let text = "function y = f(x)\n% vim: set ft=objc:\n";
    assert_eq!(detect("a.m", text), Some("objc"));
    let tail = "% tenmoku: language=objc\n";
    assert_eq!(
        detect_language(Some(Path::new("a.m")), "clc\n", tail),
        Some("objc")
    );
}

#[test]
fn the_sniff_keeps_the_paths_language_when_its_pick_isnt_loaded() {
    let _lock = lock();
    registered(&[("objc", &["m"])]);
    assert_all("a.m", MATLAB, "objc");
    assert_all("a.m", OBJC, "objc");
    registered(&[("matlab", &["m"])]);
    assert_all("a.m", OBJC, "matlab");
    assert_all("a.m", UNSURE, "matlab");
    assert_all("a.m", MATLAB, "matlab");
    registered(&[]);
    assert_eq!(detect("a.m", MATLAB[0]), None);
    // Without a path, nothing is sniffed.
    registered(&[("objc", &["m"]), ("matlab", &["m"])]);
    assert_eq!(detect_language(None, MATLAB[0], ""), None);
}

#[test]
fn no_other_extension_is_sniffed() {
    let _lock = lock();
    registered(&[("objc", &["m", "mm"]), ("matlab", &["m", "mat", "mlx"])]);
    assert_all("a.mm", MATLAB, "objc");
    assert_all("a.mat", OBJC, "matlab");
    assert_all("a.mlx", OBJC, "matlab");
    assert_all("a.MM", MATLAB, "objc");
    // A compound extension ending in `.m` keeps its owner, sniffed or not.
    registered(&[("objc", &["m"]), ("matlab", &["special.m"])]);
    assert_all("a.special.m", OBJC, "matlab");
    assert_all("a.special.m", UNSURE, "matlab");
    registered(&[("objc", &["special.m"]), ("matlab", &["m"])]);
    assert_all("a.special.m", MATLAB, "objc");
    registered(&[("objc", &["m", "special.m"]), ("matlab", &["m"])]);
    assert_all("a.special.M", MATLAB, "objc");
    assert_all("a.m", MATLAB, "matlab");
    // A file called `m`, or `.m`, has no extension `m`.
    assert_eq!(detect("dir/m", MATLAB[0]), None);
}

#[test]
fn a_third_claimant_of_m_is_left_alone() {
    let _lock = lock();
    // Registered first, it has the file, whatever its content.
    registered(&[("mercury", &["m"]), ("objc", &["m"]), ("matlab", &["m"])]);
    assert_all("a.m", OBJC, "mercury");
    assert_all("a.m", MATLAB, "mercury");
    assert_all("a.m", UNSURE, "mercury");
    // Registered after objc, the sniff never gives it the file.
    registered(&[("objc", &["m"]), ("mercury", &["m"])]);
    assert_all("a.m", MATLAB, "objc");
    registered(&[("matlab", &["m"]), ("mercury", &["m"])]);
    assert_all("a.m", OBJC, "matlab");
    assert_all("a.m", UNSURE, "matlab");
}

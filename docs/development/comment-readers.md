# Comment readers

[日本語](../ja/development/comment-readers.md)

This page describes how the server reads the comments of each language, and the known differences between its readers and the lexers of the languages.

## How comments are read

Rust is read with the lexer of the Rust compiler, JavaScript and TypeScript with the Oxc parser, C, C++, C#, and Python with their Tree-sitter grammars, and YAML with a YAML scanner. The other languages are read by readers of the server's own that follow the tokenizer of the language.

When adding comment extraction for a code language, write a reader of the server's own and compare the comments it finds with those of the language's own lexer, on real files and on generated texts. Where a Tree-sitter grammar is a candidate too, compare it the same way, and compare their time. Ruby and PHP use the server's own readers: on these checks they agreed with the lexers of the languages where the grammars did not, and took less than a tenth of the time. Java, Kotlin, and Swift use the server's own readers for the same reasons: they agreed with javac, Kotlin's lexer, and the Swift compiler on real files where the grammars of Kotlin and Swift did not, and took about a fiftieth of the time. The other languages in the list below, whose comments follow from a few rules of their tokenizers, use the server's own readers too. SQL has no lexer that its dialects share; its reader follows the token rules of Zed's SQL grammar, and reads the text between `$$` as SQL to find the comments in the body of a function. Keep the existing lexer for Rust and Oxc for JavaScript and TypeScript.

A reader is measured against the language's own lexer. Fix a reader where it differs from that lexer on real files or in a form that code commonly uses, where it panics, and where its time grows faster than the size of the text. Where it differs only on texts made for the purpose, and the difference only leaves a comment out or takes part of a literal as a comment, describe the difference under the language in [Rules of each language](#rules-of-each-language) instead of adding to the reader.

Go, Java, Kotlin, Swift, Ruby, PHP, HTML, Vue.js, Svelte, Astro, XML, CSS, SCSS, SQL, Lua, Zig, Protocol Buffers, GraphQL, Clojure, Haskell, OCaml, Scala, Dart, Elixir, JSON, TOML, shell script, PowerShell, Dockerfile, and makefile comments are read by the server's own readers, and YAML with the scanner of `granit-parser`. The scripts of HTML documents and component files are read by Oxc. The thread that handles requests waits while the comments of a document are read, so a reader must take time in proportion to the size of the text, whatever the text holds. The reading runs on a thread of its own with a stack of 256 MiB, because Oxc reads nesting by recursion and has no limit of its own on the depth ([oxc#21953](https://github.com/oxc-project/oxc/issues/21953)); only the part of the stack in use takes memory. Code nested more than about 100,000 levels deep still overflows it. The PowerShell and TOML readers replaced the lexers of `pwsh-parser` and `toml_parser`, which took time in proportion to the square of the size on some texts. The server's own readers read each part once, without starting again from an earlier place, and do not recurse. The shell, Dockerfile, Kotlin, Swift, Scala, Dart, Elixir, Ruby, and PHP readers keep what is open on a stack of their own, so only memory limits the depth of nesting. The Ruby reader looks up whether a word is a local variable in one set, whatever the depth of the scopes. The tests `readers_finish_repeated_pieces_in_seconds` and `readers_finish_texts_of_several_repeated_parts_in_seconds` check the time of the readers of thirty-one languages, YAML included, on repeated pieces of text; add a new reader to both. They do not reach every text, so when changing a reader, check each place that reads back or ahead of the current character.

## Rules of each language

### Rust

Rust supports `//` and `/* ... */`, which nest, and the documentation comments `///`, `//!`, `/** ... */`, and `/*! ... */`. A documentation comment is not translated together with an ordinary one. A shebang on the first line is skipped.

### JavaScript and TypeScript

JavaScript, JSX, TypeScript, and TSX support `//` and `/* ... */`, including JSDoc comments `/** ... */` and comments in JSX such as `{/* ... */}`. The file name gives the rules of modules and declarations, as for `.mjs`, `.cjs`, `.mts`, `.cts`, and `.d.ts`. Text inside strings, template literals, and regular expressions is not treated as a comment. A hashbang on the first line is skipped.

A syntax error that stops the parser hides the comments after it. Code nested more than about 100,000 levels deep, such as that many `[` in a row, stops the language server, because the parser reads each level of nesting by recursion. This also holds for the scripts of an HTML document or a component file.

### C#

C# supports `//`, `/* ... */`, `///`, and `/** ... */`. XML documentation is displayed as tagged text in Hover; the model is instructed to retain tags and attributes. There is no XML renderer or structural validation. Comments inside ambiguous syntax-error regions are skipped.

### C and C++

C/C++ supports ordinary and Doxygen comments (`///`, `//!`, `/**`, `/*!`, including trailing `<` forms). Documentation commands such as `@param` remain in the displayed text; the model is instructed to retain commands and parameter names. Syntax-error regions are skipped, so recognized comments after macros or incomplete expressions remain available. If an error region contains a quote or block-comment delimiter, extraction stops there to avoid treating unfinished literal contents as comments.

### Python

Python supports `#` comments and docstrings at the beginning of a module, class, or function body. Ordinary, raw, Unicode, parenthesized, and implicitly concatenated string docstrings are supported; assigned strings, f-strings, and bytes are not automatic targets. Quotes and string prefixes are removed from docstrings; source escapes and indentation are passed through without evaluating Python. Shebangs are skipped. Syntax-error regions are skipped. If an error region contains a quote, extraction stops there to avoid translating apparent comments inside unfinished strings.

### Go

Go supports `//` and `/* ... */`. Text inside strings, runes, and raw strings is not treated as a comment. Directives are skipped by the rule of Go's own `go/ast`: a `//` comment whose text right after the slashes is `line `, `extern `, `export `, or lowercase letters and digits followed by a colon and another such character, as in `//go:build` and `//nolint:errcheck`. An unclosed string or rune ends at the end of its line. An unclosed raw string or block comment hides everything after it.

### Java

Java supports `//` and `/* ... */`, and the documentation comments `/** ... */` and `///`. Consecutive `///` lines are translated together, apart from `//` lines. Text inside strings, text blocks, and character literals is not treated as a comment. An unclosed string or character literal ends at the end of its line. An unclosed text block or block comment hides everything after it.

Java reads Unicode escapes such as `\u002F` before it reads tokens; the server does not. A comment written with them is not found, and a quote written as one neither opens nor closes a string.

### Kotlin

Kotlin supports `//` and `/* ... */`, which nest, including KDoc comments `/** ... */`. Text inside strings, raw strings `"""..."""`, character literals, and names between backticks is not treated as a comment. A comment inside `${ ... }` of a string is a comment; in a string that starts with several `$`, such as `$$"..."`, a template starts with as many `$`. A shebang on the first line is skipped. An unclosed string or character literal ends at the end of its line. An unclosed raw string or block comment hides everything after it.

### Swift

Swift supports `//` and `/* ... */`, which nest, including the documentation comments `///` and `/** ... */`. Text inside strings, multi-line strings, raw strings such as `#"..."#`, regular expressions between `#/` and `/#`, and names between backticks is not treated as a comment. A comment inside `\( ... )` of a string is a comment. A shebang on the first line is skipped. An unclosed string ends at the end of its line. An unclosed multi-line string or block comment hides everything after it. An unclosed `#/` hides the rest of its line, or everything after it when only blanks and a line break follow `#/`.

A regular expression between single slashes, as in `/a+/`, is not followed, because Swift decides whether `/` starts one by the code before it. A quote inside one can hide the comments after it on its line.

### Ruby

Ruby supports `#` comments and `=begin` ... `=end` blocks. A shebang on the first line is skipped, and so is everything after `__END__`.

Text inside strings, symbols, regular expressions, `%` literals such as `%w[...]`, character literals such as `?#`, and here-documents is not treated as a comment. A comment inside `#{ ... }` of a string is a comment.

Whether `/`, `%`, `?`, and `<<` start a literal depends on what precedes them, and Ruby decides it partly by whether a word is a local variable. The server follows the same rule. A word is a local variable within its scope after it has been assigned, also as one of the targets in `a, b = list`; named as a parameter, but not in a default value; named as the variable of `for` or of `rescue => error`; bound in a pattern after `in`; or named by a group of a regular expression before `=~`, as in `/(?<year>\d+)/ =~ text`. A `def`, `class`, or `module` starts a scope that sees no variable outside it, and a block or lambda a scope that sees those outside it. In `x /2 # note`, for example, `# note` is a comment when `x` is a local variable, and part of a regular expression when `x` is a method.

The server follows the forms of these declarations that code commonly uses. A variable bound by `=>` after a value, as in `value => name`, is not followed, and in rarer forms the server can take a variable for a method or the reverse. A comment after `/`, `%`, `?`, or `<<` can then be left out, or part of a literal taken as a comment. An unclosed string, literal, here-document, or `=begin` hides everything after it.

### PHP

PHP supports `#`, `//`, `/* ... */`, and `/** ... */` in the code between `<?php`, `<?=`, or `<?` and `?>`. `<?` opens code as it does with PHP's default `short_open_tag`. The text outside the code, such as HTML, is not read, so an HTML comment there is not an automatic target.

`#[` starts an attribute, not a comment. `?>` ends a line comment as well as the code. Text inside strings, here-documents, and nowdocs is not treated as a comment. A comment inside `{$ ... }` of a string is a comment. Everything after `__halt_compiler` is skipped, unless the word names a property or a variable, as in `$object->__halt_compiler`. An unclosed string, here-document, or block comment hides everything after it.

### HTML and CSS

HTML supports `<!-- ... -->`. Inside `<script>`, the comments of JavaScript are read as in a JavaScript file, and inside `<style>`, those of CSS. A script is read when its `type` is empty, a JavaScript type, or `module`. Other scripts, such as JSON, are not. A syntax error that stops the reading of a script hides the comments after it in that script, as in a JavaScript file.

Text inside tags and their attribute values, and inside `<textarea>`, `<title>`, and the other elements whose text holds no markup, is not treated as a comment. A script ends where a browser ends it, also where `<!--` and `<script>` inside it keep a `</script>` in its text.

The server reads HTML without building the tree of the document. Where the tree changes how a browser reads the text, the server reads it as in the body of a document, with these exceptions. `<![CDATA[` runs to `]]>`, as in SVG and MathML, also elsewhere. The text of `<script>` and `<style>` inside SVG and MathML is read as JavaScript and CSS. An element such as `<textarea>` inside `<select>`, which a browser ignores there, still holds no comment. An unclosed comment or tag hides everything after it, and so does `<plaintext>`.

CSS supports `/* ... */`. Text inside strings and inside an unquoted `url( ... )` is not treated as a comment. An unclosed comment hides everything after it.

### SCSS

SCSS supports `//` and `/* ... */`, also inside a value or a selector, as in `margin: 0 // note`. Sass keeps `//` in the value of a custom property, as in `--x: a // b`; the server takes it for a comment there too. Consecutive `///` lines, the documentation comments of SassDoc, are translated together, apart from `//` lines; `////` starts an ordinary comment. Text inside strings, the strings inside `#{ ... }`, and an unquoted `url( ... )`, as in `url(//example.com/a.png)`, is not treated as a comment. A string inside `#{ ... }` that is itself inside a string of the same quotes, as in `"#{"//"}"`, ends the outer string, so a `//` or `/*` in it is taken for a comment. A backslash makes the character after it part of a name, so `\//` starts no comment. An unclosed block comment hides everything after it.

### Vue.js, Svelte, and Astro

A component file is read part by part. `<!-- ... -->` in the markup is a comment, as in HTML, and text inside tags and their attribute values, and inside `<textarea>`, is not. The comments of a script are read as in a JavaScript or TypeScript file, and those of a style sheet as in CSS. A syntax error that stops the reading of a script hides the comments after it in that script.

A script is read when it has no `lang` or its `lang` is `js`, `javascript`, `ts`, `typescript`, `jsx`, or `tsx`, and when its `type` is empty, a JavaScript type, or `module`. A style sheet is read when it has no `lang` or its `lang` is `css` or `postcss`, and with `//` comments as well when it is `scss` or `less`. Other languages, such as Sass, Stylus, and CoffeeScript, are not read.

In a Vue file, the `<template>` block, unless another language such as Pug is named by its `lang`, is read as markup, and so are the comments between blocks. Custom blocks such as `<i18n>` are not read. Text inside `{{ ... }}` is not treated as a comment, and comments of the expressions there or in the attributes of directives are not found. `{{` opens this text also inside `v-pre`.

In a Svelte or Astro file, the comments of JavaScript are read in the `{ ... }` expressions of the markup and of attributes, such as `{/* note */}`, also in Svelte's block tags such as `{#if a /* note */}`. Text inside the strings and template literals there is not treated as a comment. A comment in a tag outside `{ ... }`, which Svelte allows, is not found. Svelte's `<title>` holds markup.

In an Astro file, the script between the `---` lines is read as TypeScript, and so is `<script>`, whatever is inside it. A `---` inside a string or a comment of the script does not end it. The script is found when only text and comments come before it. Elements inside an expression, such as `{items.map((item) => <li><!-- note --></li>)}`, are read as markup. The text of an element with `is:raw` holds no comment. As the Astro compiler does, the server reads no regular expressions in an expression, so a quote in one opens a string. A backslash in a string there escapes the next character, as in JavaScript, also where the compiler does not follow it.

An unclosed comment, tag, expression, or `{{ ... }}` hides everything after it, and so does nesting of elements and expressions more than 512 levels deep.

### SQL

SQL is read by the token rules of Zed's SQL extension, whatever the dialect. It supports `--` and `/* ... */`, which do not nest. Text inside strings in single quotes, and inside names in double quotes or backticks, is not treated as a comment. A quote doubled inside them stands for itself, and in `E'...'` a backslash escapes the next character. A quote or `/*` that is never closed opens no string or comment, so the comments after it remain available.

Text between `$$` or `$tag$`, such as the body of a PostgreSQL function, is read as SQL, so the comments in a function body are found. A string written between `$$` that holds a quote or `--` can therefore hide the comments after it or have part of its text taken as a comment.

The forms of other dialects are not followed. A MySQL `#` comment is not found. MySQL escapes a quote with a backslash before it; the server closes the string there. A `/*` inside a block comment does not nest as it does in PostgreSQL.

### XML

XML supports `<!-- ... -->`, also in the internal subset of a document type declaration. Text inside tags and their attribute values, CDATA sections, processing instructions such as `<?xml ... ?>`, and the declarations of a document type is not treated as a comment.

The server does not check that a document is well formed. A comment that XML refuses, such as one that holds `--`, is still read up to its `-->`, and a `<` inside an attribute value, which XML refuses too, opens nothing. An unclosed comment, CDATA section, processing instruction, or tag hides everything after it.

### Lua

Lua supports `--` comments, and long comments such as `--[[ ... ]]` and `--[==[ ... ]==]`, which run to the closing bracket with as many `=`. Consecutive `---` lines, the documentation comments of LuaCATS, are translated together, apart from `--` lines. Text inside strings and long strings such as `[[ ... ]]` is not treated as a comment; in a string, a backslash at the end of a line and `\z` continue it on the next line. As `lua` does with a file, a first line that starts with `#` is skipped. An unclosed string ends at the end of its line. An unclosed long string or long comment hides everything after it.

### Zig

Zig supports `//` comments, the documentation comments `///`, and the container documentation comments `//!`; `////` starts an ordinary comment. Consecutive `///` or `//!` lines are translated together, apart from `//` lines. Text inside strings, character literals, and the lines of multiline strings, which start with `\\`, is not treated as a comment. Zig ends lines at LF only, so a CR alone does not end a comment. An unclosed string or character literal ends at the end of its line.

### Protocol Buffers

Protocol Buffers supports `//` and `/* ... */`, which do not nest, as `protoc` reads them. Text inside strings in single or double quotes is not treated as a comment. An unclosed string ends at the end of its line. An unclosed block comment hides everything after it.

### GraphQL

GraphQL supports `#` comments. Text inside strings and block strings `"""..."""` is not treated as a comment. A description is a string, so Hover does not translate it; select it and use `Translate selection to …`. An unclosed string ends at the end of its line. An unclosed block string hides everything after it.

### Clojure

Clojure supports `;` comments. Every `;` at the start of a comment, up to 255, is part of its mark, so consecutive `;;` lines are translated together, apart from `;` lines. A comment ends at CR or LF, as Clojure's reader ends it. A `;` inside a form that `#_` discards, or inside `(comment ...)`, starts a comment; the forms of `(comment ...)` themselves are code. Text inside strings, regular expressions `#"..."`, and character literals such as `\;`, `\"`, and `\newline` is not treated as a comment, and neither is a docstring. `#!` starts a comment to the end of the line, which is skipped, as a shebang is. A `#!` inside a symbol or a keyword, as in `a#!b`, starts nothing. Neither does one right after a number, as in `1#!x`, where Clojure starts a comment. An unclosed string or regular expression hides everything after it.

A character literal that Clojure refuses, such as `\\#!`, runs to the end of its token, as a valid one does, so a `#!` in it starts no comment.

### Haskell

Haskell supports `--` comments and `{- ... -}` block comments, which nest, as GHC reads them with no language extension. Two or more `-` start a comment only where they are not part of an operator: `-->`, `|--`, and `--|` are operators, and `---` and `-- |` start comments. A comment ends at LF; the CR of a CRLF is not part of it, and a CR alone does not end it. Every `-` at the start of a comment, up to 255, is part of its mark, so consecutive `---` lines are translated together, apart from `--` lines.

The Haddock comments `-- |` and `-- ^` are translated together with the `--` lines right after them, and without their `|` and `^`, as are `{-|`, `{-^`, `{- |`, and `{- ^`. `{-#` starts a pragma, which is not a comment; the text after it is read as code, as GHC reads the pragmas it knows, so a comment inside `{-# RULES ... #-}` is found. Text inside strings and character literals such as `'"'` is not treated as a comment; `'` after a name, as in `foldl'`, and before one, as in `'name` and `''Type`, starts no character literal. A quasi-quotation such as `[name| ... |]` and the lines of the C preprocessor are read as code, as GHC reads them without the extensions. An unclosed string ends at the end of its line. An unclosed block comment hides everything after it.

GHC decides whether a character outside ASCII belongs to an operator by its Unicode category. The server counts every such character that is neither a letter, a digit, nor whitespace as part of an operator, except the common brackets and quotation marks such as `「`, `（`, `“`, and `«`, which GHC does not count. A comment written right after `--` with another such character, as in `--⟨note⟩`, is therefore not found. In a pragma that GHC does not know, such as a misspelled `{-# INLNE f #-}`, GHC reads a comment; the server reads code there, so a `--` inside it can be taken as a comment.

### OCaml

OCaml and OCaml interfaces support `(* ... *)` comments, which nest, and the documentation comments `(** ... *)`; `(***` starts an ordinary comment, and `(*)` starts a comment, as it does for OCaml. Inside a comment, OCaml reads strings, quoted strings such as `{|...|}` and `{id|...|id}`, and character literals, so a `*)` inside one of them does not end the comment, and a `"` in `'"'` starts no string. Outside comments, text inside these literals, also inside `{%ext|...|}`, is not treated as a comment. `'` after a name, as in `x'`, and before one, as in the type variable `'a`, starts no character literal. The `*` after the opening `(` and before the closing `)` of a comment is not translated. An unclosed comment, string, or quoted string hides everything after it.

In the delimiter of a quoted string, every letter outside ASCII counts as lowercase, and the closing delimiter must have the same bytes. So `{É|`, which opens no quoted string for OCaml, opens one here, and one that OCaml closes with another Unicode form of the same letters stays open; both hide the comments after them.

### Scala

Scala supports `//` and `/* ... */`, which nest, including Scaladoc comments `/** ... */`. `//` and `/*` start a comment also right after an operator, as in `a +// note`. Text inside strings, strings of three quotes `"""..."""`, character literals, and names between backticks is not treated as a comment. A name right before a quote, such as `s` in `s"..."`, starts an interpolated string, unless it is a keyword such as `if`; a comment inside `${ ... }` of an interpolated string is a comment, and `$$` and `$"` stand for `$` and `"`. `'` before a name or an operator, as in `'name`, and before the `{` or `[` of a quote starts no character literal; a comment inside `'{ ... }` is a comment. A line that starts with `#!` is read as code, as Scala's lexer reads it. An unclosed string ends at the end of its line. An unclosed string of three quotes or block comment hides everything after it.

The server does not follow XML literals: the text of one, as in `<a>// x</a>`, is read as code, so a `//`, `/*`, or quotation mark in it is taken as it would be in code.

Scala counts some characters outside ASCII as part of a name where the server does not: a symbol such as `→` after the `_` of a name, and a combining mark or formatting character that is not a letter, such as U+0301 or U+200C. The server ends the name there. So it can miss the interpolated string that such a name starts, as in `a_→"..."`, and with it the comments inside its `${ ... }`. In rare forms it starts one that Scala does not, and takes text inside its `${ ... }` as a comment. A name right after a number, as in `1s"..."`, starts no interpolated string here either.

### Dart

Dart supports `//` and `/* ... */`, which nest, including the documentation comments `///` and `/** ... */`. Consecutive `///` lines are translated together, apart from `//` lines. Text inside strings, strings of three quotes such as `'''...'''`, and raw strings such as `r'...'` is not treated as a comment. A comment inside `${ ... }` of a string that is not raw is a comment. A script tag `#!` on the first line is skipped. An unclosed string ends at the end of its line. An unclosed string of three quotes or block comment hides everything after it.

Where Dart reports an error, the server does not follow how Dart reads on. A string left unclosed inside `${ ... }` hides the rest of its line, or everything after it when it has three quotes; Dart takes that quote for the end of the outer string instead, so it finds the comments that follow. Around other unmatched quotes inside `${ ... }`, and at an `r` before a quote right after an unfinished number such as `0x` or `1e`, the server can miss a comment or take text inside a string as one.

### Elixir

Elixir supports `#` comments. Text inside strings, charlists such as `'...'`, heredocs `"""..."""` and `'''...'''`, sigils such as `~r/#\d+/` and `~S(...)`, character literals such as `?#` and `?"`, and quoted atoms and keys such as `:"a#"` and `"a#": 1` is not treated as a comment. A comment inside `#{ ... }` of a string, a charlist, a heredoc, a quoted atom or key, or a sigil with a lowercase name is a comment. A `#!` line at the start of the file is skipped. An unclosed string, charlist, heredoc, quoted atom, or sigil hides everything after it.

The string, heredoc, or sigil `~S` or `~s` given to `@moduledoc`, `@doc`, or `@typedoc` is translated as a docstring, also in parentheses, as in `@doc("...")`. Its text is sent as written, escapes and `#{ ... }` included; a comment inside its `#{ ... }` is not translated on its own. When an operator follows, as in `@doc "a" <> "b"`, the first string is the doc. `@doc false`, `@shortdoc`, a charlist, another sigil, and a string on the line after `@doc` are not docs. An unclosed doc is not translated. A quoted key right after one of the attributes, as in `@doc "a": 1`, is not a doc, but a comment inside its `#{ ... }` is left out.

### JSON, YAML, and TOML

JSON and JSONC support `//` and `/* ... */`; YAML and TOML support `#`. Only comments are automatic targets. To translate a key or a string value, select it and use `Translate selection to …`.

Text inside a string is not treated as a comment: JSON strings, quoted YAML scalars and block scalars (`|` and `>`), and the four kinds of TOML strings. In YAML, a `#` that does not follow whitespace, as in `a#b` or `http://a/#frag`, is not a comment either.

These formats are read token by token. In JSON and TOML, errors in the structure, such as a missing comma, an unfinished value, or an unclosed bracket, therefore do not hide comments. An unclosed one-line string ends at the end of its line: its contents are not read as comments, and later lines are unaffected. An unclosed JSON block comment, or an unclosed multi-line TOML string, hides everything after it. In JSON, text in single quotes is skipped like a string. In TOML, a quote in the middle of a bare value, as in `don't`, also starts a string. In YAML, extraction stops at the first error the scanner reports, such as an unclosed quote, a tab used for indentation, or a line that is neither a key nor a value. Comments before the error remain available; one on the line of the error, or just before it, can be missing.

### Shell scripts

Shell scripts support `#` comments. A shebang on the first line is skipped. sh, Bash, and zsh scripts are read in the same way. Where the shells read a script differently, the server follows Bash, and leaves a comment out rather than take text that Bash reads as data.

A `#` starts a comment at the start of a line, and after a blank, `;`, or `&`. A `#` inside a word, as in `a#b` or `${x#y}`, does not. Text inside quotes, here-documents, `${ ... }`, and arithmetic such as `$(( ... ))` and `(( ... ))` is not treated as a comment. A comment inside a command substitution, `$( ... )` or backquotes, is a comment, also when the substitution is inside a quoted string. A substitution inside a here-document is not read.

The following comments are not automatic targets:

- A comment right after `(`, `)`, `|`, `<`, or `>`, with no blank between. Bash reads a comment there, and zsh can read a part of a pattern, as in `(#b)` and `(a|b)#`.
- Inside backquotes, a comment that the closing backquote ends, and every comment after a backslash followed by `\`, a backquote, `$`, or `"`.

The structure of a script is not checked, so an `if` without its `fi`, or a `)` without its `(`, hides nothing. An unclosed quote, `${`, or `$((` hides everything after it, and so does a here-document without its ending line.

Extraction stops at the following, where the server cannot tell how a shell reads on. The comments before that point remain available.

- `${ command; }`, which Bash 5.3 reads as commands and older shells do not.
- A here-document whose delimiter holds `$` or a backquote outside single quotes, or has `(` right after it.
- A here-document that is still open at the `)` of the `$( ... )` around it.
- A backslash at the end of a line that splits an operator, such as `&&`, `<<`, or `$(`, over two lines.
- `((` that opens two subshells instead of arithmetic, when the text up to the first `)` that balances holds `#`, `[`, `<<`, or `case`.
- `<<` inside the parentheses of a pattern, as in `@(a|<<b)`, where zsh can begin a here-document.
- Inside `$( ... )`, a `case` pattern in parentheses with a word right after it, as in `(a)echo`, and a `)` that closes the substitution before the `esac` of a `case` in it.

### PowerShell

PowerShell supports `#` comments and `<# ... #>` block comments. A shebang on the first line and `#Requires` lines are skipped.

A `#` or `<#` starts a comment at the start of a line, after a blank, and after `(`, `)`, `{`, `}`, `,`, `;`, `|`, or `&`. It also does right after a string or a block comment that began at one of these places. A `#` after a backtick does not start a comment. Text inside single-quoted strings, double-quoted strings, here-strings, and `${ ... }` is not treated as a comment. The other quotation marks that PowerShell accepts, such as `‘`, `’`, `“`, and `”`, work as `'` and `"` do. A comment inside `$( ... )` within a double-quoted string is not an automatic target.

Where a `#` follows other text with no blank between, as in `a#b`, `$x#text`, or `${x}#text`, PowerShell reads a comment or a part of an argument, depending on the statement around it. The server does not decide between the two, and nothing on that line from the `#` on is an automatic target. The same holds for the rest of a line after `--%`, which a command takes as it is written, also where the `--%` ends a string.

The server follows PowerShell where it reads on although the text looks finished:

- In a double-quoted string and in a here-string in double quotes, `$( ... )` runs to its last parenthesis and `${ ... }` to its `}`, over quotes and over a line that would end the here-string.
- A backtick before a line break inside an argument, or inside a here-string in double quotes, escapes the line break, so the next line does not start a token or end the here-string. After CRLF it does, because the backtick takes only the CR.
- A here-string with other text than blanks after its opening `@"` or `@'` ends at `"@` or `'@` on that line, or with that line.
- `$$` and `@$` are one variable each, so `$$(` and `@${` open no `$( ... )` or `${ ... }`.

An unclosed string, here-string, or `${` hides the comments after it.

Extraction stops at the following, where the server cannot tell how PowerShell reads on. The comments before that point remain available.

- `<#` or a here-string right after other text, as in `$x=@"`.
- `$( ... )` right after other text, as in `a=$( ... )`, when it holds `#`, a backtick, `${`, `--%`, a here-string, or a string with a parenthesis in it.
- A backtick before a quote, `$`, `@`, `<`, `-`, or `u{`, when it follows a letter, a digit, `_`, `.`, `\`, `+`, `#`, or another backtick, as in ``a`"b``, or when its word holds `-name`, as in ``-a=`"b``. A type name such as ``List`1`` and a parameter of a command hold a backtick that escapes nothing.
- `${ ... }` in a word that holds `-name`, or after `@` inside a word, when it holds a quote, `#`, a backtick, or `%`. PowerShell can read its `{` as the start of a block there.
- A quote, `#`, `$`, `@`, `<`, a backtick, or one of `(`, `)`, `{`, `}`, `;`, `|`, and `&` after `[name,` on the same line. PowerShell can read the name of an assembly there, in which these open nothing. A backtick inside `[name` stops extraction too, before anything but a letter or a digit.
- `--%` with `|` or `&&` after it on its line, and `--%` after such a `#` or another `--%` on its line.
- A string, here-string, block comment, or `${ ... }` that starts after such a `#` or `--%` on its line and ends on a later line. The same holds for `$( ... )` right after other text, and for a backtick right after other text at the end of that line.

### Dockerfile and Docker Compose

In a Dockerfile, a comment is a line whose first character after its indentation is `#`. A `#` later in a line, as in `FROM a # text`, is not a comment. Comment lines between the continued lines of an instruction are comments.

Parser directives are skipped: the lines `# syntax=...`, `# escape=...`, and `# check=...` at the top of the file, before any other comment, blank line, or instruction. A shebang on the first line is skipped too.

The lines of a here-document are not comments. A here-document follows a `RUN`, `COPY`, or `ADD` instruction that has a word starting with `<<` or `<<-`, and runs to the line that holds only its delimiter. With `<<-`, tabs before the delimiter on that line are ignored. A here-document without that line runs to the end of the file. `<<<` does not start a here-document, and neither does `<<` inside quotes.

The delimiter is read as BuildKit reads it. Quotes around it or around a part of it are removed. A backslash is removed outside quotes and kept in single quotes. In double quotes it is removed only before `"`, `$`, or another backslash, so the delimiter of `<<"E\OF"` is `E\OF`. The escape directive does not change this.

BuildKit has no here-document in the following places, and reads the lines after them as instructions and comments. The server reads a here-document there as well, and takes a line as a comment only where both readings do.

- `<<-` followed by a blank and a word, as in `<<- EOF`. The word is the delimiter.
- An instruction with an unclosed quote, or with a `${` that BuildKit rejects, after a word that starts with `<<`. That word gives the delimiter.
- An empty delimiter, as in `<<""`. The here-document ends at an empty line.
- A delimiter that holds `<`, as in `<<"a<b"`.

Extraction stops where the delimiter cannot be read: where the unclosed quote or the rejected `${` is in the word of `<<` or in its delimiter, and where that word holds a `${name ... }` with more than a name, as in `<<EOF${x:-y}`. The comments before that point remain available.

Docker Compose files are read as YAML, by the rules under [JSON, YAML, and TOML](#json-yaml-and-toml).

### Makefiles

Makefiles are read as GNU Make 4.3 and later read them. A `#` starts a comment that runs to the end of the line, and a backslash at the end of that line continues the comment on the next line. A `#` after a backslash, as in `\#`, and a `#` inside a variable reference or function call, such as `$(shell echo '#')`, start no comment. The body of `define` is the value of a variable, so its `#` lines are not comments.

A recipe line, which starts with a tab after a rule, goes to the shell. Its comments are read as in a shell script, after the `@`, `-`, and `+` at its start: a `# note` alone on the line, or after a command, as in `cc -c a.c # note`. Text inside the quotes of the shell is not treated as a comment. The text after the `;` that starts a recipe on the line of its rule is not read, so a comment there is not found.

GNU Make 3.81, which macOS ships, ends a variable reference or function call at a `#` inside it, as a comment. The server follows the later versions, where such a `#` starts no comment. A recipe prefix other than a tab, set with `.RECIPEPREFIX`, is not followed. Lines end at LF only.

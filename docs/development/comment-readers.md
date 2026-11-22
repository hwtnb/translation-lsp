# Comment readers

[日本語](../ja/development/comment-readers.md)

This page describes how the server reads the comments of each language, and the known differences between its readers and the lexers of the languages.

## How comments are read

| Reader | Languages |
| --- | --- |
| Rust compiler lexer | Rust |
| Oxc parser | JavaScript and TypeScript, including scripts in HTML and component files |
| Tree-sitter grammars | C, C++, C#, Python |
| `granit-parser` scanner | YAML, including Docker Compose |
| Server's own readers | Go, Java, Kotlin, Swift, Ruby, PHP, HTML, Vue.js, Svelte, Astro, XML, CSS, SCSS, SQL, Lua, Zig, Protocol Buffers, GraphQL, Clojure, Haskell, OCaml, Scala, Dart, Elixir, Terraform, Terraform Vars, HCL, Nix, Prisma, Gleam, Erlang, CMake, GLSL, F#, JSON, TOML, shell scripts, PowerShell, Dockerfile, makefiles |

When adding comment extraction for a code language, write a reader and compare the comments it finds with those of the language's lexer on real files and generated texts. Where a Tree-sitter grammar is a candidate too, compare its comments and reading time in the same way. Keep the existing lexer for Rust and Oxc for JavaScript and TypeScript.

The readers for Ruby and PHP agreed with the languages' lexers where the grammars differed and used less than a tenth of the time. The readers for Java, Kotlin, and Swift agreed with javac, Kotlin's lexer, and the Swift compiler, respectively, on real files and used about a fiftieth of the time. The Kotlin and Swift grammars differed on some of those files.

The server uses its own readers for the other languages because a few rules of their tokenizers determine which text is a comment.

SQL has no lexer that its dialects share. Its reader follows the token rules of Zed's SQL grammar and reads the text between `$$` as SQL to find comments in function bodies.

A reader is measured against the language's own lexer. Fix a reader where it differs from that lexer on real files or in a form that code commonly uses, where it panics, and where its time grows faster than the size of the text.

Where it differs only on texts made for the purpose, and the difference only leaves a comment out or takes part of a literal as a comment, describe the difference under the language in [Rules of each language](#rules-of-each-language) instead of adding to the reader.

The thread that handles requests waits while the comments of a document are read, so a reader must take time in proportion to the size of the text, whatever the text holds.

The reading runs on a thread with a stack of 256 MiB because Oxc uses recursion with no limit of its own on the depth. Only the part of the stack in use takes memory. Code nested more than about 100,000 levels deep can still overflow the stack. [oxc#21953](https://github.com/oxc-project/oxc/issues/21953)

The PowerShell and TOML readers replaced the lexers of `pwsh-parser` and `toml_parser`, which took time in proportion to the square of the size on some texts.

The server's own readers read each part once, without restarting from an earlier place or using recursion. The readers below keep open constructs on their own stacks, so available memory alone limits nesting depth.

| Readers with their own stack |
| --- |
| Shell scripts, Dockerfile, Kotlin, Swift, Scala, Dart, Elixir, Ruby, PHP, HCL, Nix, F# |

The Ruby reader looks up whether a word is a local variable in one set, whatever the depth of the scopes.

The tests `readers_finish_repeated_pieces_in_seconds` and `readers_finish_texts_of_several_repeated_parts_in_seconds` each time 39 language IDs, including YAML, on repeated pieces of text. Add a new reader to both. The tests cover a selection of inputs, so when changing a reader, check each place that reads back or ahead of the current character.

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

### Terraform and HCL

Terraform, Terraform Vars, and HCL share a reader. Line comments start with `#` or `//` and end at LF or CRLF. A bare CR stays in the comment. A block comment starts at `/*` and ends at the first `*/`. An unclosed block hides everything after it.

Literal text in double-quoted strings and heredocs is skipped. `${ ... }` and `%{ ... }` contain code, so comments inside those expressions are read. `$${` and `%%{` stay literal. A backslash in a double-quoted string escapes the next character. A single quote starts no string.

A heredoc opens with `<<` or `<<-`, a name, and a line ending immediately after the name. Its closing line contains the same name with optional whitespace on either side. When a template expression ends partway through a line, that line cannot close the heredoc.

An unfinished quoted string or heredoc keeps its literal text hidden. Within it, comments in template expressions still count.

Heredoc names use the Unicode identifier classes available in the Rust lexer. In rare cases, HCL accepts a name that those classes reject, such as `ͺ` (U+037A). In that case, a line in the heredoc body can be taken for a comment. A double quote in the body can also open a string that hides comments after the heredoc.

Compare the reader with the `hclsyntax` lexer from hashicorp/hcl. Its line comment tokens include the trailing LF or CRLF, so remove that ending before comparing ranges. The lexer continues past an unclosed `/*` without treating it as a comment. Skip those inputs when comparing ranges.

### Nix

Line comments start with `#` and end at CR or LF. Block comments run from `/*` to the first `*/`. `/**` opens a documentation comment unless the next character is `/` or `*`. The text sent for translation leaves out the opening and closing marks. An unclosed block hides everything after it.

Literal text in double-quoted strings and indented strings (`'' ... ''`) is skipped. Comments inside `${ ... }` are read. In both kinds of string, `$${` stays literal. In an indented string, `'''`, `''$`, and `''\` with the next character are escapes.

`//` is the update operator. Apostrophes and hyphens can be part of a name, as in `x'` and `a-b`. URIs can contain `/*` without starting a comment, as in `x:/*c*/x`. Their tokens end before `#`, as in `http://a/b#c`. Paths such as `./a` can contain interpolation, where comments are read as in strings.

When an indented string immediately follows a decimal number with an exponent, as in `[ 1.2e3''# literal'' ]`, the reader takes the text from the `#` inside the string to the end of the line as one comment.

The reader follows Nix's `lexer.l`. When comparing with rnix, account for the fact that rnix ends a `#` comment only at LF. Use LF for bare CR in the comparison input and trim the trailing CR from CRLF comment ranges. An unclosed `/*` starts no comment in `lexer.l` and causes a lexer error in rnix.

### Prisma

The reader finds comments anywhere outside double-quoted strings. Line comments begin with `//` or the documentation marker `///` and end at CR, LF, or the end of the text. Consecutive documentation lines are grouped separately from ordinary lines. Block comments run from `/*` to the first `*/`. An unfinished block hides the rest of the document.

In a string, a backslash escapes the next character on the line. Prisma's `datamodel.pest` grammar accepts a backslash followed by a single LF or CR inside a string. The reader ends the string at the line break, so `//` or `/*` in the remaining text can start a comment. The closing quote can open another string and hide a later comment on that line. An apparent `/*` without a matching `*/` hides all later comments.

Compare byte ranges only in files where the grammar uses neither `CATCH_ALL` nor `BLOCK_LEVEL_CATCH_ALL`. For recognized blocks such as `model`, the grammar skips the rest of the opening line after `{`. Add any comments it skips there before comparing. Exclude files where a block comment starts on that line and continues onto another line, since the grammar parses the following lines separately.

Exclude files with an unrecognized block (`arbitrary_block`), whose body the grammar skips up to the first `}`. Also exclude files with an unfinished block comment.

### Gleam

Line comments begin with `//`, `///` for documentation, or `////` for module documentation. Any slashes after the fourth belong to the body. Adjacent lines can form a group when their opening markers have the same length. Comments end at CR, LF, or the end of the text.

Double-quoted strings can span lines, and a backslash escapes the next character. An unfinished string hides the rest of the document. Compare byte ranges with the Gleam lexer, starting each range at the first slash of the comment.

### Erlang

Line comments start at `%` and end before LF or CRLF. A bare CR stays in the comment. Adjacent standalone comments can form a group when they have the same indentation and the same number of opening percent signs. The reader treats up to 255 percent signs as the opening and keeps any more in the body. A shebang on the first line is skipped.

Percent signs inside strings, quoted atoms, character literals such as `$%`, and sigils are part of those literals. A sigil with a one-character delimiter closes at its first unescaped closing delimiter without nesting. With these delimiters, ordinary strings, quoted atoms, and the sigils `~`, `~s`, and `~b` process escapes. An unfinished literal hides the rest of the file. A tilde without a sigil delimiter is skipped.

A multiline string opens with at least three double quotes followed by whitespace up to LF. It closes at the same number of quotes after optional whitespace at the start of a line. Only `~s` and `~b` process escapes inside a multiline string. When non-whitespace follows the opening quotes on the same line, the reader treats them as ordinary string delimiters.

At the start of a form, the first string given to `-doc` or `-moduledoc` is translated, with or without one opening parenthesis. A form starts at the beginning of the file or after a single period followed by whitespace, a comment, or EOF. The documentation can use an ordinary or multiline string, or a sigil named `~`, `~s`, `~S`, `~b`, or `~B`.

The documentation range covers the delimiters and excludes the sigil prefix and suffix. Its body keeps source escapes and shares the indentation trimming used by Elixir documentation. Atoms, maps, macros, and file references used as documentation values are skipped. The reader also skips documentation attributes with quoted names such as `-'doc'`.

Compare comment and documentation byte ranges with `erl_scan` using the `return_comments` and `text` options. For CRLF comments, remove the final CR from the scanner's range before comparing. Before scanning, replace the shebang line up to its line ending with one space per byte. The reader treats `.xrl` files as Erlang, so quotes or percent signs in leex regular expressions may affect comment detection.

### CMake

A `#` starts a comment that ends before the next LF or CRLF, or at the end of the file. A CR alone stays in the comment. A bracket comment begins with `#[[` or with equals signs between the opening brackets. It ends at the first closing bracket pair with the same number of equals signs. Code on the rest of that line is read as usual.

A bracket argument starts only at a token boundary. It can contain comment marks and has no escapes. Double-quoted arguments can span lines, and a backslash escapes the following character. In unquoted arguments, a backslash can escape any character except a line break. Unclosed quoted arguments, bracket arguments, and bracket comments hide the rest of the file.

Compare the byte ranges with those from `cmListFileLexer`. Keep offsets in the original bytes, including any UTF-8 byte order mark. For line comments that end at CRLF, leave the CR out of the range.

### GLSL

A `//` comment ends at LF, CRLF, or CR. A `/*` comment ends at the first `*/`, and an unclosed one hides the rest of the file. A backslash followed by a line break joins the lines, including inside a comment. Comments inside an inactive preprocessor branch are read as usual.

Double quotes start a string. Within it, a backslash escapes the following character, and an unescaped line break ends an unfinished string. A single quote is an ordinary character.

The reader joins lines in every version. In older GLSL profiles, glslang may end a `//` comment at a line break even after a backslash. A line splice between the two characters of an opening comment mark is unsupported. A block comment can close across a splice. In an angle-bracket `#include` path, `//` can be read as a comment.

Compare byte ranges with glslang's preprocessor on files it accepts. Compare only comments from the main source, even when `#line` changes the reported name. glslang counts offsets from the byte after a leading UTF-8 byte order mark. Determine line and file ends before adding the mark's length to the offsets. Before comparing, account for line comments that glslang ends early in older profiles and for the unsupported opening and include forms above.

### F#

Line comments begin with `//`, `///` for XML documentation, or `////`, and end at CR, LF, or EOF. Adjacent lines group only when their opening lengths match, with four or more slashes sharing length 4. Because F# reads operator characters as one token, `//` immediately after an operator character, as in `x +// y`, starts no comment. A leading `#!` line is skipped.

Block comments run from `(*` to matching `*)` and nest. `(*)` is an operator and neither starts nor nests a comment. Block bodies drop leading and trailing `*` marks, as in OCaml. Inside a block comment, strings (`"..."`, `@"..."`, `"""..."""`) and simple character literals keep `*)` from closing the comment; numeric and Unicode escapes do not count as character literals there.

Outside comments, the reader skips strings, character literals (including numeric and Unicode escapes), identifiers containing `'`, and backtick identifiers. An unclosed comment or string, including an unclosed interpolated string, hides the rest of the file.

In interpolated strings (`$"..."`, `$@"..."`, `@$"..."`, `$"""..."""`, and multi-`$` forms such as `$$"""..."""`), comments inside interpolation holes are read as code. In a multi-`$` string, a hole opens with at least as many `{` as `$` and closes with that many `}`.

Preprocessor lines such as `#if`, `#else`, `#endif`, and `#nowarn` are read as code after `#`, so comments on those lines and in every branch are read. Lines starting with `#:` are skipped to the end of the line.

A `#!` outside the first line is read as ordinary characters rather than a line comment. Because every `#if` branch is read as code, an unclosed string or block comment in an inactive branch hides the rest of the file. Non-ASCII identifier membership uses Rust's Unicode letter and digit checks, which can differ from F# on combining or formatting characters. A `#line` path ending in `\"` without `@` leaves a string unclosed.

Compare byte ranges against comment trivia from `FSharp.Compiler.Service` on files without end-of-file lexical errors. Before parsing, replace `///` with `//!` and bare CR with LF so documentation and CR-ended comments are recorded. Replace `#` with a space on `#nowarn`, `#warnon`, and conditional directive lines until none remain, exposing every branch and trailing comment. Skip a leading `#!` line and count offsets from byte 0, including any UTF-8 byte order mark.

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

use std::borrow::Cow;

use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Range};
use uuid::Uuid;

use crate::{
    config::hover::{HoverFormat, HoverSettings},
    providers::dictionary::inline,
    store::Key,
};

pub(super) mod cache;
pub(super) mod files;
pub(super) mod markdown;

use markdown::Urls;

#[derive(Clone)]
pub(crate) struct Preview<C> {
    id: uuid::Uuid,
    pub context: C,
    pub range: Range,
    pub translation: String,
    pub form: Form,
    pub is_comment: bool,
    /// The provider's line under the Hover title.
    pub note: Option<&'static str>,
    /// The `hover` settings that the translation completed under.
    pub hover: HoverSettings,
    /// The keys the translation is stored under.
    pub keys: Vec<Key>,
}

/// What a translation is, which decides how Hover and its CodeLens show it.
#[derive(Clone)]
pub(crate) enum Form {
    /// A dictionary entry in Markdown, with the line that a CodeLens shows
    /// for it, cut to fit.
    Entry(String),
    /// The translation of a Markdown document, which is Markdown, with the
    /// URLs of its original.
    Markdown(Urls),
    /// Any other translation, with the URLs of its original.
    Text(Urls),
}

impl<C> Preview<C> {
    pub fn new(
        context: C,
        range: Range,
        translation: String,
        is_comment: bool,
        note: Option<&'static str>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            context,
            range,
            translation,
            form: Form::Text(Urls::default()),
            is_comment,
            note,
            hover: HoverSettings::default(),
            keys: Vec::new(),
        }
    }

    /// The Hover at `position`, for a client that reads Markdown as
    /// `markup` says.
    pub fn hover(&self, position: Position, markup: Markup) -> Option<Hover> {
        // Zed keyboard hover uses the selection head, including the exclusive end.
        // Cached ranges match their own positions; there is no nearest-range fallback.
        if position < self.range.start || position > self.range.end {
            return None;
        }
        let body = self.body(markup);
        let value = if self.hover.header {
            let header = header(TITLE, self.note, markup.theme_icons);
            format!("{header}\n\n---\n\n{body}")
        } else {
            body
        };
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(self.range),
        })
    }

    /// The Markdown that Hover shows under its header, for a client that
    /// reads Markdown as `markup` says.
    pub fn body(&self, markup: Markup) -> String {
        let body = match (&self.form, self.hover.format) {
            (Form::Entry(_), _) => self.translation.clone(),
            // A link to elsewhere would be one the original does not have.
            (Form::Markdown(urls), HoverFormat::Markdown) => {
                markdown::document(&self.translation, &linkable(urls, markup), markup.marked)
                    .unwrap_or_else(|| fenced_text(&self.translation))
            }
            (Form::Text(urls), HoverFormat::Markdown) => {
                markdown::render(&self.translation, &linkable(urls, markup))
            }
            (Form::Markdown(_) | Form::Text(_), HoverFormat::Text) => {
                fenced_text(&self.translation)
            }
        };
        shown(body, markup.theme_icons)
    }

    /// The text whose first line a CodeLens shows: the term and its first
    /// meaning for a dictionary entry, and the translation otherwise.
    pub fn lens_text(&self) -> &str {
        match &self.form {
            Form::Entry(line) => line,
            Form::Markdown(_) | Form::Text(_) => &self.translation,
        }
    }

    /// The translation as the Markdown of an editor message. Zed wraps the
    /// lines of a message but not those of a code block in it, so the text is
    /// escaped instead of fenced.
    pub fn message(&self) -> String {
        if matches!(self.form, Form::Entry(_)) {
            self.translation.clone()
        } else {
            plain_text(&self.translation)
        }
    }
}

/// The title of the Hover of a translation.
const TITLE: &str = "Translation LSP";

/// What a client does with Markdown beyond what Zed does.
#[derive(Clone, Copy, Default)]
pub(crate) struct Markup {
    /// It draws `$(name)` as an icon, as VS Code does.
    pub theme_icons: bool,
    /// It reads Markdown with marked, as VS Code does, which also makes links
    /// of `www.` and of e-mail addresses in text.
    pub marked: bool,
}

/// The URLs of the original that links may open in a client that reads
/// Markdown as `markup` says. In a client that draws theme icons, a URL with
/// `$(` is no link: escaping its icon name would change where it leads.
fn linkable(urls: &Urls, markup: Markup) -> Cow<'_, Urls> {
    if markup.theme_icons {
        Cow::Owned(urls.without_icon_names())
    } else {
        Cow::Borrowed(urls)
    }
}

/// The header of a Hover that the server shows, which starts with `title`,
/// with `note` under it. A client that draws theme icons gets the title after
/// an icon, as a line of its own, where a quote would indent it; any other
/// client gets it as a quote.
pub(crate) fn header(title: &str, note: Option<&str>, icons: bool) -> String {
    match (icons, note) {
        (false, None) => format!("> **{title}**"),
        (false, Some(note)) => format!("> **{title}**\n>\n> {note}"),
        (true, None) => format!("$(globe) {title}"),
        (true, Some(note)) => format!("$(globe) {title}\n\n{}", shown(note.into(), true)),
    }
}

/// `markdown` as a client that draws theme icons if `icons` shows it as
/// written.
pub(crate) fn shown(markdown: String, icons: bool) -> String {
    if icons {
        literal_icons(&markdown)
    } else {
        markdown
    }
}

/// `markdown` with a word joiner after each `$` that a `(` follows, directly
/// or after a backslash. VS Code draws `$(name)` as an icon wherever it is in
/// the rendered Markdown, including code spans, where no backslash escapes
/// it, so the invisible character keeps the two apart in every context.
fn literal_icons(markdown: &str) -> String {
    let mut shown = String::with_capacity(markdown.len());
    for (index, character) in markdown.char_indices() {
        shown.push(character);
        if character == '$' {
            let rest = &markdown[index + 1..];
            if rest.starts_with('(') || rest.starts_with("\\(") {
                shown.push('\u{2060}');
            }
        }
    }
    shown
}

/// `label`, a line of plain text such as the title of a CodeLens, as a
/// client that draws theme icons if `icons` shows it as written. VS Code draws
/// `$(name)` in a label as an icon unless a backslash comes before it, and
/// shows a backslash anywhere else, so only icon names get one.
pub(crate) fn label_shown(label: String, icons: bool) -> String {
    if !icons {
        return label;
    }
    let mut shown = String::with_capacity(label.len());
    for (index, character) in label.char_indices() {
        if character == '$' && icon_name_at(&label[index + 1..]) {
            shown.push('\\');
        }
        shown.push(character);
    }
    shown
}

/// Whether `text` starts with the rest of an icon name that VS Code reads
/// after a `$`: `(`, letters, digits, and `-`, an optional `~` modifier of
/// letters, and `)`.
fn icon_name_at(text: &str) -> bool {
    let Some(rest) = text.strip_prefix('(') else {
        return false;
    };
    let name = rest
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'-')
        .count();
    if name == 0 {
        return false;
    }
    let rest = &rest[name..];
    let rest = match rest.strip_prefix('~') {
        Some(modifier) => {
            let letters = modifier.bytes().take_while(u8::is_ascii_alphabetic).count();
            if letters == 0 {
                return false;
            }
            &modifier[letters..]
        }
        None => rest,
    };
    rest.starts_with(')')
}

/// `text` as Markdown that shows the words of each line as written, with the
/// line breaks of `text`.
pub(super) fn plain_text(text: &str) -> String {
    text.lines()
        .map(|line| {
            let line = inline(line);
            // A line of `=` under a line of text would make it a heading.
            if line.starts_with('=') {
                format!("\\{line}")
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("  \n")
}

fn fenced_text(text: &str) -> String {
    let mut longest = 0;
    let mut current = 0;
    for character in text.chars() {
        current = if character == '`' { current + 1 } else { 0 };
        longest = longest.max(current);
    }
    let fence = "`".repeat(3.max(longest + 1));
    format!("{fence}text\n{text}\n{fence}")
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    use crate::providers::chatgpt::test_support::Fixture;

    use super::*;

    #[test]
    fn messages_escape_markdown_and_keep_line_breaks() {
        let text = "**retry** `count`\n\n===\n- item\nhttps://example.com";
        assert_eq!(
            plain_text(text),
            "\\*\\*retry\\*\\* \\`count\\`  \n  \n\\===  \n\\- item  \nhttps:\u{2060}//example.com"
        );
        let markdown = plain_text(text);
        let mut parsed = Parser::new(&markdown).filter_map(|event| match event {
            Event::Text(text) => Some(text.into_string()),
            Event::HardBreak => Some("\n".into()),
            Event::Start(Tag::Heading { .. } | Tag::List(_) | Tag::CodeBlock(_)) => {
                panic!("Unexpected {event:?}")
            }
            _ => None,
        });
        assert_eq!(parsed.next().unwrap(), "*");
        assert!(parsed.any(|text| text == "\n"));
    }

    /// The text of `markdown` when it is one paragraph in Zed, without the
    /// word joiners that keep URLs from being links.
    fn zed_text(markdown: &str) -> Option<String> {
        let mut text = String::new();
        for event in Parser::new_ext(markdown, markdown::ZED_OPTIONS) {
            match event {
                Event::Text(part) => text.push_str(&part),
                Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => (),
                _ => return None,
            }
        }
        Some(text.replace('\u{2060}', ""))
    }

    #[test]
    fn messages_show_dashes_quotes_and_dots_as_written() {
        for (text, markdown) in [
            ("--verbose", "\\-\\-verbose"),
            ("---", "\\-\\-\\-"),
            ("a - b", "a - b"),
            ("It's \"quoted\"...", "It\\'s \\\"quoted\\\"\\.\\.\\."),
            ("1.5", "1\\.5"),
            ("e.g.", "e.g."),
            ("a..b", "a..b"),
            (
                "`--verbose` を付けると、各試行を表示する。",
                "\\`\\-\\-verbose\\` を付けると、各試行を表示する。",
            ),
        ] {
            assert_eq!(inline(text), markdown);
            assert_eq!(zed_text(markdown).as_deref(), Some(text), "{markdown:?}");
        }
        // Lines of pieces from a fixed sequence of pseudorandom numbers.
        const PIECES: &[&str] = &[
            "-", "--", ".", "...", "'", "\"", "a", "語", " ", "1", "e.g.", "+", ")", "*", "`", "<",
            ":", "//", "x://y", "[", "]", "(", "!", "#", "=", "|", "~", "_", "&", ";", "\\",
        ];
        let started = Instant::now();
        let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut next = |bound: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % bound as u64) as usize
        };
        for _ in 0..2000 {
            let length = 1 + next(12);
            let text: String = (0..length).map(|_| PIECES[next(PIECES.len())]).collect();
            let markdown = inline(&text);
            let written = text.split_whitespace().collect::<Vec<_>>().join(" ");
            assert_eq!(
                zed_text(&markdown).as_deref(),
                Some(written.as_str()),
                "{text:?} → {markdown:?}"
            );
        }
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn fences_preserve_whitespace_identifiers_and_markdown() {
        let source = "\t`retry_count`  🚀\r\n````\r\n<script>**text**</script>";
        assert_eq!(fenced_text(source), format!("`````text\n{source}\n`````"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn header_and_rule_leave_both_provider_bodies_literal() {
        let fixture = Fixture::new().await;
        let auth = fixture.auth();
        fixture.connected(&auth, 3600).await;
        let source =
            "\t`id` 🚀\r\n````\r\n<script>**text**</script> ![image](https://example.invalid/)";
        for (note, format) in [
            (Some("Mock translation (fixed sample)"), HoverFormat::Text),
            (None, HoverFormat::Text),
            (
                Some("Mock translation (fixed sample)"),
                HoverFormat::Markdown,
            ),
            (None, HoverFormat::Markdown),
        ] {
            let mut preview = Preview::new(
                auth.key().unwrap(),
                Range::new(Position::new(0, 0), Position::new(0, 1)),
                source.into(),
                false,
                note,
            );
            preview.hover.format = format;
            let value = value(&preview);
            assert!(value.starts_with("> **Translation LSP**"));
            assert!(!value.contains("ChatGPT 翻訳"));
            assert_eq!(
                value.contains("Mock translation (fixed sample)"),
                note.is_some()
            );
            let body = match format {
                HoverFormat::Text => fenced_text(source),
                HoverFormat::Markdown => markdown::render(source, &Urls::default()),
            };
            assert!(value.ends_with(&format!("\n\n---\n\n{body}")));
            let events: Vec<_> = Parser::new(&value).collect();
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event, Event::Rule))
                    .count(),
                1
            );
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(event, Event::Start(Tag::BlockQuote(_))))
                    .count(),
                1
            );
            assert!(
                events
                    .iter()
                    .any(|event| matches!(event, Event::Start(Tag::CodeBlock(_))))
            );
            assert!(!events.iter().any(|event| matches!(
                event,
                Event::Html(_) | Event::InlineHtml(_) | Event::Start(Tag::Image { .. })
            )));
        }
        assert!(fixture.state.lock().unwrap().records.is_empty());
    }

    fn value<C>(preview: &Preview<C>) -> String {
        let HoverContents::Markup(content) = preview
            .hover(preview.range.start, Markup::default())
            .unwrap()
            .contents
        else {
            panic!("Expected Markdown");
        };
        content.value
    }

    #[test]
    fn hover_settings_choose_the_header_and_the_form_of_the_body() {
        let translation = "`a_b` を使う。\n次の行";
        let mut preview = Preview::new(
            (),
            Range::default(),
            translation.into(),
            false,
            Some("Note"),
        );
        let header = "> **Translation LSP**\n>\n> Note\n\n---\n\n";
        let markdown = "`a_b` を使う。\\\n次の行";
        let fenced = "```text\n`a_b` を使う。\n次の行\n```";
        for (shows_header, format, expected) in [
            (true, HoverFormat::Markdown, format!("{header}{markdown}")),
            (false, HoverFormat::Markdown, markdown.into()),
            // The format of earlier versions, unchanged.
            (true, HoverFormat::Text, format!("{header}{fenced}")),
            (false, HoverFormat::Text, fenced.into()),
        ] {
            preview.hover = HoverSettings {
                header: shows_header,
                format,
            };
            assert_eq!(value(&preview), expected);
        }
        // An entry is Markdown in both formats, and so is the translation of a
        // Markdown document in Markdown.
        for (form, format, body) in [
            (
                Form::Entry(String::new()),
                HoverFormat::Markdown,
                translation,
            ),
            (Form::Entry(String::new()), HoverFormat::Text, translation),
            (
                Form::Markdown(Urls::default()),
                HoverFormat::Markdown,
                translation,
            ),
            (Form::Markdown(Urls::default()), HoverFormat::Text, fenced),
        ] {
            preview.form = form;
            preview.hover = HoverSettings {
                header: false,
                format,
            };
            assert_eq!(value(&preview), body);
        }
    }

    /// How VS Code reads Markdown, apart from marked.
    const ICONS: Markup = Markup {
        theme_icons: true,
        marked: false,
    };

    #[test]
    fn headers_take_the_form_that_the_client_shows_well() {
        for (note, icons, expected) in [
            (None, false, "> **Translation LSP**"),
            (Some("Note"), false, "> **Translation LSP**\n>\n> Note"),
            (None, true, "$(globe) Translation LSP"),
            (Some("Note"), true, "$(globe) Translation LSP\n\nNote"),
            (
                Some("Uses $(sync)"),
                true,
                "$(globe) Translation LSP\n\nUses $\u{2060}(sync)",
            ),
        ] {
            assert_eq!(header("Translation LSP", note, icons), expected);
        }
    }

    #[test]
    fn icon_names_stay_as_written_only_for_a_client_with_theme_icons() {
        for (markdown, expected) in [
            ("$(sync)", "$\u{2060}(sync)"),
            ("`$(file)` and $(x)", "`$\u{2060}(file)` and $\u{2060}(x)"),
            ("\\$(sync)", "\\$\u{2060}(sync)"),
            ("$\\(sync\\)", "$\u{2060}\\(sync\\)"),
            ("```text\n$(sync)\n```", "```text\n$\u{2060}(sync)\n```"),
            ("$ (sync) $$ 5$", "$ (sync) $$ 5$"),
            ("末尾 $", "末尾 $"),
        ] {
            assert_eq!(shown(markdown.into(), true), expected, "{markdown:?}");
            assert_eq!(shown(markdown.into(), false), markdown);
        }
        // Every form of the body, and the note under the header; the
        // translation itself stays as it is.
        let translation = "Run `$(sync)`, then $(file).";
        let mut preview = Preview::new(
            (),
            Range::default(),
            translation.into(),
            false,
            Some("$(x)"),
        );
        for (form, format) in [
            (Form::Text(Urls::default()), HoverFormat::Markdown),
            (Form::Text(Urls::default()), HoverFormat::Text),
            (Form::Markdown(Urls::default()), HoverFormat::Markdown),
            (Form::Entry(String::new()), HoverFormat::Markdown),
        ] {
            preview.form = form;
            preview.hover.format = format;
            let HoverContents::Markup(content) =
                preview.hover(preview.range.start, ICONS).unwrap().contents
            else {
                panic!("Expected Markdown");
            };
            let icons = content.value;
            let body = icons
                .strip_prefix("$(globe) Translation LSP\n\n$\u{2060}(x)\n\n---\n\n")
                .unwrap_or_else(|| panic!("{icons:?}"));
            assert_eq!(body, preview.body(ICONS));
            // Markdown escapes `(` in text, so `$\(` is broken up too.
            assert_eq!(body.matches("$\u{2060}").count(), 2, "{body:?}");
            assert!(!body.contains("$(") && !body.contains("$\\("), "{body:?}");
            assert_eq!(
                body.replace('\u{2060}', ""),
                preview.body(Markup::default())
            );
            assert_eq!(preview.translation, translation);
        }
    }

    #[test]
    fn a_client_that_reads_markdown_with_marked_gets_no_link_that_the_original_lacks() {
        let original = "Read the manual.";
        let urls = Urls::of_markdown(original);
        let vscode = Markup {
            theme_icons: true,
            marked: true,
        };
        for (translation, marked) in [
            (
                "マニュアルは www.example.com で読む。",
                "マニュアルは www\u{2060}.example.com で読む。",
            ),
            (
                "質問は person@example.org に送る。",
                "質問は person@\u{2060}example.org に送る。",
            ),
            (
                "WWW.Example.com と a.b+c@d-e.org",
                "WWW\u{2060}.Example.com と a.b+c@\u{2060}d-e.org",
            ),
            // Code, and an `@` that no address surrounds, stay as written.
            (
                "`www.example.com` と @name と a@ b",
                "`www.example.com` と @name と a@ b",
            ),
        ] {
            let mut preview = Preview::new((), Range::default(), translation.into(), false, None);
            preview.form = Form::Markdown(urls.clone());
            assert_eq!(preview.body(vscode), marked, "{translation:?}");
            // Zed makes no link of them, and gets the translation as it was.
            assert_eq!(preview.body(Markup::default()), translation);
        }
    }

    /// The forms of the two reviews of the first fixes, each with what VS Code
    /// and Zed get. In none does marked, which VS Code reads Markdown with,
    /// find a link that the original lacks: footnotes show as text, other
    /// links outside the original make the whole translation a code block,
    /// and `://`, `www.`, and e-mail addresses that either reading has as
    /// text get a word joiner. Zed gets what it got before.
    #[test]
    fn the_translation_of_a_markdown_document_links_only_where_the_original_does_as_marked_reads_it()
     {
        let original = "Read [the docs](https://example.com/d) at <https://example.com> and \
            [notes](#notes).\n\n## Notes\n";
        let vscode = Markup {
            theme_icons: true,
            marked: true,
        };
        for (translation, marked, zed) in [
            (
                "詳細は http://evil.example と HTTPS://evil.example と ftp://evil.example を読む。",
                "詳細は http:\u{2060}//evil.example と HTTPS:\u{2060}//evil.example と ftp:\u{2060}//evil.example を読む。",
                "詳細は http:\u{2060}//evil.example と HTTPS:\u{2060}//evil.example と ftp:\u{2060}//evil.example を読む。",
            ),
            (
                "<http://evil.example> を読む。",
                "```text\n<http://evil.example> を読む。\n```",
                "```text\n<http://evil.example> を読む。\n```",
            ),
            (
                "<evil@example.org> に送る。",
                "```text\n<evil@example.org> に送る。\n```",
                "```text\n<evil@example.org> に送る。\n```",
            ),
            (
                "<www.evil.example> を読む。",
                "<www\u{2060}.evil.example> を読む。",
                "<www.evil.example> を読む。",
            ),
            (
                "[文書][1] を読む。\n\n[1]: https://evil.example",
                "```text\n[文書][1] を読む。\n\n[1]: https://evil.example\n```",
                "```text\n[文書][1] を読む。\n\n[1]: https://evil.example\n```",
            ),
            (
                "段落の文。\n[1]: https://evil.example\n\n[1] を読む。",
                "段落の文。\n[1]: https:\u{2060}//evil.example\n\n[1] を読む。",
                "段落の文。\n[1]: https:\u{2060}//evil.example\n\n[1] を読む。",
            ),
            (
                "> [1]: https://evil.example\n\n[1] を読む。",
                "```text\n> [1]: https://evil.example\n\n[1] を読む。\n```",
                "```text\n> [1]: https://evil.example\n\n[1] を読む。\n```",
            ),
            (
                "- [1]: https://evil.example\n\n[1] を読む。",
                "```text\n- [1]: https://evil.example\n\n[1] を読む。\n```",
                "```text\n- [1]: https://evil.example\n\n[1] を読む。\n```",
            ),
            (
                "| a |\n| - |\n| b |\n[1]: https://evil.example\n\n[1] を読む。",
                "| a |\n| - |\n| b |\n[1]: https:\u{2060}//evil.example\n\n[1] を読む。",
                "| a |\n| - |\n| b |\n[1]: https:\u{2060}//evil.example\n\n[1] を読む。",
            ),
            (
                "注[^1]を見る。\n\n[^1]: https:evil.example",
                "注\\[^1]を見る。\n\n\\[^1]: https:evil.example",
                "注[^1]を見る。\n\n[^1]: https:evil.example",
            ),
            (
                "注[^a]\n\n[^a]: mailto:evil%40example.org",
                "注\\[^a]\n\n\\[^a]: mailto:evil%40example.org",
                "注[^a]\n\n[^a]: mailto:evil%40example.org",
            ),
            (
                "注[^a]\n\n[^a]: file:/etc/hosts",
                "注\\[^a]\n\n\\[^a]: file:/etc/hosts",
                "注[^a]\n\n[^a]: file:/etc/hosts",
            ),
            (
                "## www.evil.example の evil@example.org\n\n| www.evil.example |\n| - |\n| evil@example.org |\n\n*www.evil.example*",
                "## www\u{2060}.evil.example の evil@\u{2060}example.org\n\n| www\u{2060}.evil.example |\n| - |\n| evil@\u{2060}example.org |\n\n*www\u{2060}.evil.example*",
                "## www.evil.example の evil@example.org\n\n| www.evil.example |\n| - |\n| evil@example.org |\n\n*www.evil.example*",
            ),
            (
                "mailto:evil@example.org と xmpp:evil@example.org",
                "mailto:evil@\u{2060}example.org と xmpp:evil@\u{2060}example.org",
                "mailto:evil@example.org と xmpp:evil@example.org",
            ),
            (
                "https://example.com.evil.org を読む。",
                "https:\u{2060}//example.com.evil.org を読む。",
                "https:\u{2060}//example.com.evil.org を読む。",
            ),
            (
                "<span>www.evil.example</span>\n\n<div>\nhttps://evil.example\n</div>",
                "<span>www\u{2060}.evil.example</span>\n\n<div>\nhttps:\u{2060}//evil.example\n</div>",
                "<span>www.evil.example</span>\n\n<div>\nhttps:\u{2060}//evil.example\n</div>",
            ),
            (
                "![www.evil.example の evil@example.org](https://example.com/d)",
                "![www.evil.example の evil@example.org](https://example.com/d)",
                "![www.evil.example の evil@example.org](https://example.com/d)",
            ),
            (
                "## 見出し {#id data=www.evil.example}",
                "## 見出し {#id data=www\u{2060}.evil.example}",
                "## 見出し {#id data=www.evil.example}",
            ),
            (
                "## 見出し {.note data=https://evil.example}",
                "## 見出し {.note data=https:\u{2060}//evil.example}",
                "## 見出し {.note data=https://evil.example}",
            ),
            (
                "[文書](https://example.com/d) と <https://example.com> と [注](#notes)",
                "[文書](https://example.com/d) と <https://example.com> と [注](#notes)",
                "[文書](https://example.com/d) と <https://example.com> と [注](#notes)",
            ),
        ] {
            let mut preview = Preview::new((), Range::default(), translation.into(), false, None);
            preview.form = Form::Markdown(Urls::of_markdown(original));
            assert_eq!(preview.body(vscode), marked, "{translation:?}");
            assert_eq!(preview.body(Markup::default()), zed, "{translation:?}");
        }
    }

    /// A footnote, which marked has none of and would read as a link, shows
    /// as text in VS Code, and the rest keeps its formatting. Zed gets the
    /// footnote as it was.
    #[test]
    fn footnotes_show_as_text_where_markdown_is_read_with_marked() {
        let vscode = Markup {
            theme_icons: true,
            marked: true,
        };
        for (original, translation, marked) in [
            (
                "# Guide\n\nSee the guide[^1].\n\n[^1]: guide.md\n",
                "# ガイド\n\nガイドを見る[^1]。\n\n[^1]: guide.md\n",
                "# ガイド\n\nガイドを見る\\[^1]。\n\n\\[^1]: guide.md\n",
            ),
            (
                "# Notes\n\nText[^1].\n\n[^1]: Details.\n",
                "# 説明\n\n本文[^1]。\n\n[^1]: 詳細は付録を参照。\n",
                "# 説明\n\n本文\\[^1]。\n\n\\[^1]: 詳細は付録を参照。\n",
            ),
            (
                "# Notes\n\nText[^1].\n\n[^1]: For details see the appendix.\n",
                "# 注\n\n本文[^1]。\n\n[^1]: 詳細は付録を参照。\n",
                "# 注\n\n本文\\[^1]。\n\n\\[^1]: 詳細は付録を参照。\n",
            ),
        ] {
            let mut preview = Preview::new((), Range::default(), translation.into(), false, None);
            preview.form = Form::Markdown(Urls::of_markdown(original));
            assert_eq!(preview.body(vscode), marked, "{translation:?}");
            assert_eq!(preview.body(Markup::default()), translation);
        }
    }

    #[test]
    fn a_link_to_a_section_with_an_icon_name_is_text_in_a_client_with_theme_icons() {
        let markdown = "See [the notes](#notes-$(file)) and [the rest](#rest).";
        let mut preview = Preview::new((), Range::default(), markdown.into(), false, None);
        preview.form = Form::Markdown(Urls::of_markdown(markdown));
        assert_eq!(preview.body(Markup::default()), markdown);
        let body = preview.body(ICONS);
        assert!(body.starts_with("```text\n"), "{body:?}");
        let plain = "See [the rest](#rest).";
        preview.translation = plain.into();
        assert_eq!(preview.body(ICONS), plain);
    }

    #[test]
    fn a_link_with_an_icon_name_is_text_in_a_client_with_theme_icons() {
        let url = "https://example.org/$(file)";
        let markdown = format!("See [the guide]({url}).");
        let mut preview = Preview::new((), Range::default(), markdown.clone(), false, None);
        preview.form = Form::Markdown(Urls::of_markdown(&markdown));
        assert_eq!(preview.body(Markup::default()), markdown);
        let body = preview.body(ICONS);
        assert!(body.starts_with("```text\n"), "{body:?}");
        assert!(!body.contains(url), "{body:?}");

        let text = format!("See {url} for the guide.");
        let mut preview = Preview::new((), Range::default(), text.clone(), false, None);
        preview.form = Form::Text(Urls::of(&text));
        assert!(
            preview
                .body(Markup::default())
                .contains(&format!("<{url}>"))
        );
        let body = preview.body(ICONS);
        assert!(!body.contains('<') && !body.contains(url), "{body:?}");
        // A URL without an icon name stays a link.
        let text = "See https://example.org/guide for the guide.";
        let mut preview = Preview::new((), Range::default(), text.into(), false, None);
        preview.form = Form::Text(Urls::of(text));
        assert!(preview.body(ICONS).contains("<https://example.org/guide>"));
    }

    #[test]
    fn labels_escape_only_icon_names_and_only_for_a_client_with_theme_icons() {
        for (label, shown) in [
            ("$(sync) を押す", "\\$(sync) を押す"),
            ("$(sync~spin) と $(file)", "\\$(sync~spin) と \\$(file)"),
            (
                "$(echo hi) $() $(a~) $(a b) $ (x) $",
                "$(echo hi) $() $(a~) $(a b) $ (x) $",
            ),
            ("費用 $(5) と $(a-1)。", "費用 \\$(5) と \\$(a-1)。"),
        ] {
            assert_eq!(label_shown(label.into(), true), shown, "{label:?}");
            assert_eq!(label_shown(label.into(), false), label);
        }
    }

    #[test]
    fn hover_includes_only_its_own_range_and_exact_end() {
        let range = Range::new(Position::new(1, 4), Position::new(2, 35));
        let preview = Preview::new((), range, "模擬訳".into(), false, None);
        for position in [range.start, Position::new(2, 10), range.end] {
            assert_eq!(
                preview.hover(position, Markup::default()).unwrap().range,
                Some(range)
            );
        }
        assert!(
            preview
                .hover(Position::new(1, 3), Markup::default())
                .is_none()
        );
        assert!(
            preview
                .hover(Position::new(2, 36), Markup::default())
                .is_none()
        );
    }
}

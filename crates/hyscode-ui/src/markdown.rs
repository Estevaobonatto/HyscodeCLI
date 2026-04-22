//! Renderização de Markdown no terminal com syntax highlighting.
//!
//! Usa pulldown-cmark para parse e syntect para syntax highlighting de blocos de código.

use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use syntect::{
    easy::HighlightLines,
    highlighting::ThemeSet,
    parsing::SyntaxSet,
    util::LinesWithEndings,
};
use console::style;

static THEME_SET: std::sync::LazyLock<ThemeSet> = std::sync::LazyLock::new(ThemeSet::load_defaults);
static SYNTAX_SET: std::sync::LazyLock<SyntaxSet> = std::sync::LazyLock::new(SyntaxSet::load_defaults_newlines);

/// Renderiza uma string Markdown para o terminal.
/// Retorna a string com formatação ANSI aplicada.
pub fn render_markdown(input: &str) -> String {
    let mut output = String::new();
    let parser = Parser::new(input);

    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buffer = String::new();
    let mut in_bold = false;
    let mut in_italic = false;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(lang)) => {
                in_code_block = true;
                code_lang = match lang {
                    CodeBlockKind::Fenced(lang_str) => lang_str.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                code_buffer.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                output.push('\n');
                output.push_str(&highlight_code(&code_buffer, &code_lang));
                output.push('\n');
                code_lang.clear();
            }
            Event::Start(Tag::Strong) => {
                in_bold = true;
            }
            Event::End(TagEnd::Strong) => {
                in_bold = false;
            }
            Event::Start(Tag::Emphasis) => {
                in_italic = true;
            }
            Event::End(TagEnd::Emphasis) => {
                in_italic = false;
            }
            Event::Start(Tag::Heading { level, .. }) => {
                output.push('\n');
                let prefix = "#".repeat(level as usize);
                output.push_str(&style(format!("{} ", prefix)).cyan().bold().to_string());
            }
            Event::End(TagEnd::Heading(_)) => {
                output.push('\n');
            }
            Event::Start(Tag::List(_)) => {}
            Event::End(TagEnd::List(_)) => {}
            Event::Start(Tag::Item) => {
                output.push_str("  • ");
            }
            Event::End(TagEnd::Item) => {
                output.push('\n');
            }
            Event::Text(text) => {
                if in_code_block {
                    code_buffer.push_str(&text);
                } else {
                    let mut s = text.to_string();
                    if in_bold {
                        s = style(s).bold().to_string();
                    }
                    if in_italic {
                        s = style(s).italic().to_string();
                    }
                    output.push_str(&s);
                }
            }
            Event::Code(code) => {
                output.push_str(&style(format!(" `{}` ", code)).dim().to_string());
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                output.push('\n');
            }
            Event::Start(Tag::BlockQuote(_)) => {
                output.push_str(&style("> ").dim().to_string());
            }
            Event::End(TagEnd::BlockQuote) => {
                output.push('\n');
            }
            Event::Start(Tag::Link { .. }) => {}
            Event::End(TagEnd::Link) => {}
            Event::HardBreak | Event::SoftBreak => {
                output.push('\n');
            }
            Event::Html(html) => {
                output.push_str(&html);
            }
            _ => {}
        }
    }

    output.trim().to_owned()
}

/// Syntax highlighting de código usando syntect.
fn highlight_code(code: &str, lang: &str) -> String {
    let theme = &THEME_SET.themes["base16-ocean.dark"];
    let syntax = SYNTAX_SET
        .find_syntax_by_token(lang)
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut output = String::new();

    for line in LinesWithEndings::from(code) {
        let highlighted = highlighter.highlight_line(line, &SYNTAX_SET).unwrap_or_default();
        let escaped = syntect::util::as_24_bit_terminal_escaped(&highlighted, false);
        output.push_str(&escaped);
    }

    // Reset ANSI
    output.push_str("\x1b[0m");
    output
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_markdown_bold() {
        let input = "**hello** world";
        let output = render_markdown(input);
        assert!(output.contains("hello"));
        assert!(output.contains("world"));
    }

    #[test]
    fn test_render_markdown_heading() {
        let input = "# Title\n\nparagraph";
        let output = render_markdown(input);
        assert!(output.contains("Title"));
        assert!(output.contains("paragraph"));
    }

    #[test]
    fn test_render_markdown_code_block() {
        let input = "```rust\nfn main() {}\n```";
        let output = render_markdown(input);
        assert!(output.contains("main"));
    }
}

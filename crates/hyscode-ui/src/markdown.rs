//! Renderização de Markdown no terminal com syntax highlighting.
//!
//! Duas saídas disponíveis:
//! - `render_markdown` → string com escapes ANSI (para stdout direto)
//! - `render_markdown_lines` → `Vec<Line>` do ratatui (para TUI)

use console::style;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

static THEME_SET: std::sync::OnceLock<ThemeSet> = std::sync::OnceLock::new();
static SYNTAX_SET: std::sync::OnceLock<SyntaxSet> = std::sync::OnceLock::new();

// ═══════════════════════════════════════════════════════════════════════════════
// Versão ANSI (stdout)
// ═══════════════════════════════════════════════════════════════════════════════

/// Renderiza uma string Markdown para o terminal com escapes ANSI.
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
            Event::Start(Tag::Strong) => in_bold = true,
            Event::End(TagEnd::Strong) => in_bold = false,
            Event::Start(Tag::Emphasis) => in_italic = true,
            Event::End(TagEnd::Emphasis) => in_italic = false,
            Event::Start(Tag::Heading { level, .. }) => {
                output.push('\n');
                let prefix = "#".repeat(level as usize);
                output.push_str(&style(format!("{} ", prefix)).cyan().bold().to_string());
            }
            Event::End(TagEnd::Heading(_)) => output.push('\n'),
            Event::Start(Tag::List(_)) => {}
            Event::End(TagEnd::List(_)) => {}
            Event::Start(Tag::Item) => output.push_str("  • "),
            Event::End(TagEnd::Item) => output.push('\n'),
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
            Event::End(TagEnd::Paragraph) => output.push('\n'),
            Event::Start(Tag::BlockQuote(_)) => output.push_str(&style("> ").dim().to_string()),
            Event::End(TagEnd::BlockQuote) => output.push('\n'),
            Event::Start(Tag::Link { .. }) => {}
            Event::End(TagEnd::Link) => {}
            Event::HardBreak | Event::SoftBreak => output.push('\n'),
            Event::Html(html) => output.push_str(&html),
            _ => {}
        }
    }

    output.trim().to_owned()
}

// ═══════════════════════════════════════════════════════════════════════════════
// Versão ratatui (TUI)
// ═══════════════════════════════════════════════════════════════════════════════

/// Renderiza Markdown para `Vec<Line>` do ratatui.
/// Permite estilização nativa dentro do TUI.
pub fn render_markdown_lines(input: &str, base_fg: Color, accent: Color) -> Vec<Line<'_>> {
    let parser = Parser::new(input);
    let mut lines: Vec<Line> = Vec::new();
    let mut current_spans: Vec<Span> = Vec::new();

    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_buffer = String::new();
    let mut style_stack = Style::default().fg(base_fg);

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(lang)) => {
                flush_spans(&mut lines, &mut current_spans);
                in_code_block = true;
                code_lang = match lang {
                    CodeBlockKind::Fenced(lang_str) => lang_str.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                code_buffer.clear();
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                let highlighted = highlight_code(&code_buffer, &code_lang);
                for hl_line in highlighted.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("  ".to_string(), Style::default()),
                        Span::styled(
                            hl_line.to_string(),
                            Style::default().fg(Color::Rgb(200, 200, 220)),
                        ),
                    ]));
                }
                code_lang.clear();
            }
            Event::Start(Tag::Strong) => {
                style_stack = style_stack.add_modifier(Modifier::BOLD);
            }
            Event::End(TagEnd::Strong) => {
                style_stack = style_stack.remove_modifier(Modifier::BOLD);
            }
            Event::Start(Tag::Emphasis) => {
                style_stack = style_stack.add_modifier(Modifier::ITALIC);
            }
            Event::End(TagEnd::Emphasis) => {
                style_stack = style_stack.remove_modifier(Modifier::ITALIC);
            }
            Event::Start(Tag::Heading { level, .. }) => {
                flush_spans(&mut lines, &mut current_spans);
                let prefix = "#".repeat(level as usize);
                current_spans.push(Span::styled(
                    format!("{} ", prefix),
                    Style::default().fg(accent).add_modifier(Modifier::BOLD),
                ));
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_spans(&mut lines, &mut current_spans);
            }
            Event::Start(Tag::List(_)) => {}
            Event::End(TagEnd::List(_)) => {}
            Event::Start(Tag::Item) => {
                current_spans.push(Span::styled("  • ", Style::default().fg(base_fg)));
            }
            Event::End(TagEnd::Item) => {
                flush_spans(&mut lines, &mut current_spans);
            }
            Event::Text(text) => {
                if in_code_block {
                    code_buffer.push_str(&text);
                } else {
                    current_spans.push(Span::styled(text.to_string(), style_stack));
                }
            }
            Event::Code(code) => {
                current_spans.push(Span::styled(
                    format!(" `{}` ", code),
                    Style::default()
                        .fg(Color::Rgb(180, 180, 195))
                        .add_modifier(Modifier::DIM),
                ));
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                flush_spans(&mut lines, &mut current_spans);
            }
            Event::Start(Tag::BlockQuote(_)) => {
                current_spans.push(Span::styled("> ", Style::default().fg(Color::Gray)));
            }
            Event::End(TagEnd::BlockQuote) => {
                flush_spans(&mut lines, &mut current_spans);
            }
            Event::Start(Tag::Link { .. }) => {
                current_spans.push(Span::styled("[".to_string(), Style::default().fg(base_fg)));
            }
            Event::End(TagEnd::Link) => {
                current_spans.push(Span::styled("]".to_string(), Style::default().fg(base_fg)));
            }
            Event::HardBreak | Event::SoftBreak => {
                flush_spans(&mut lines, &mut current_spans);
            }
            Event::Html(html) => {
                current_spans.push(Span::styled(html.to_string(), style_stack));
            }
            _ => {}
        }
    }

    flush_spans(&mut lines, &mut current_spans);
    lines
}

fn flush_spans<'a>(lines: &mut Vec<Line<'a>>, spans: &mut Vec<Span<'a>>) {
    if !spans.is_empty() {
        lines.push(Line::from(std::mem::take(spans)));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Syntax highlighting
// ═══════════════════════════════════════════════════════════════════════════════

fn highlight_code(code: &str, lang: &str) -> String {
    let theme_set = THEME_SET.get_or_init(ThemeSet::load_defaults);
    let syntax_set = SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines);

    let theme = &theme_set.themes["base16-ocean.dark"];
    let syntax = syntax_set
        .find_syntax_by_token(lang)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut output = String::new();

    for line in LinesWithEndings::from(code) {
        let highlighted = highlighter
            .highlight_line(line, syntax_set)
            .unwrap_or_default();
        let escaped = syntect::util::as_24_bit_terminal_escaped(&highlighted, false);
        output.push_str(&escaped);
    }

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

    #[test]
    fn test_render_markdown_lines_basic() {
        let input = "**bold** and *italic*";
        let lines = render_markdown_lines(input, Color::White, Color::Cyan);
        assert!(!lines.is_empty());
    }
}

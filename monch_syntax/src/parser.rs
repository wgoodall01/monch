use crate::ast;
use pest::{
    iterators::{Pair, Pairs},
    Span,
};
use pest_derive::Parser;

/// Generate a parser from the grammar in `monch.pest`
/// This also generates the `Rule` enum, in the scope of the module.
#[derive(Parser)]
#[grammar = "monch.pest"]
pub struct PestParser;

pub type Error = pest::error::Error<Rule>;

pub type Result<T> = std::result::Result<T, Error>;

pub struct Parser {}

impl Parser {
    /// Create a new parser, at the origin of the input text.
    pub fn new() -> Parser {
        Parser {}
    }

    /// Parse `source` with the generated parser according to the given `rule`.
    fn parse_rule<'s>(&self, input: &'s str, rule: Rule) -> Result<Pair<'s, Rule>> {
        use pest::Parser;
        let mut result = PestParser::parse(rule, input)?;
        let found = result.next().expect("expected to parse some input");
        assert_eq!(result.next(), None, "found extra input in parse_rule");
        Ok(found)
    }

    /// Parse a Command from a given string of input
    pub fn parse_command(&self, cmd: &str) -> Result<ast::Command> {
        let parsed = self.parse_rule(cmd, Rule::CommandInput)?;

        let mut ctx = Context::unpack(parsed, Rule::CommandInput);
        let cmd = ctx.match_rule(Rule::Command);
        let _ignored_epi = ctx.match_rule(Rule::EOI);
        ctx.done();

        self.p_command(cmd)
    }

    /// Parse a Script from a given string of input
    pub fn parse_script(&self, script: &str) -> Result<ast::Script> {
        let parsed = self.parse_rule(script, Rule::ScriptInput)?;

        let mut ctx = Context::unpack(parsed, Rule::ScriptInput);

        // Parse each command in the script
        let commands = ctx
            .inner()
            .filter(|p| p.as_rule() != Rule::EOI)
            .map(|p| self.p_command(p))
            .collect::<Result<_>>()?;

        ctx.done();

        Ok(ast::Script { commands })
    }

    fn p_command(&self, input: Pair<Rule>) -> Result<ast::Command> {
        let mut ctx = Context::unpack(input, Rule::Command);

        // Get each inner invocation rule
        let inv_rules = ctx.inner().collect::<Vec<Pair<Rule>>>();
        let inv_rules_len = inv_rules.len();

        let mut invocations: Vec<ast::Invocation> = vec![];
        let mut read_redirect: Option<ast::ReadRedirect> = None;
        let mut write_redirect: Option<ast::WriteRedirect> = None;

        for (i, pair) in inv_rules.into_iter().enumerate() {
            // The first item can redirect input, the last can redirect output.
            let can_redirect_input = i == 0;
            let can_redirect_output = i == inv_rules_len - 1;

            let (inv, stdin_redir, stdout_redir) =
                self.p_invocation(pair, can_redirect_input, can_redirect_output)?;

            // Record the invocation
            invocations.push(inv);

            // Check for a stdin redirect
            if let Some(r) = stdin_redir {
                assert!(read_redirect.is_none());
                read_redirect = Some(r)
            }

            // Check for a stdout redirect
            if let Some(r) = stdout_redir {
                assert!(write_redirect.is_none());
                write_redirect = Some(r)
            }
        }

        ctx.done();

        Ok(ast::Command {
            pipeline: invocations,
            stdin_redirect: read_redirect,
            stdout_redirect: write_redirect,
        })
    }

    fn p_invocation(
        &self,
        input: Pair<Rule>,
        can_redirect_input: bool,
        can_redirect_output: bool,
    ) -> Result<(
        ast::Invocation,
        Option<ast::ReadRedirect>,
        Option<ast::WriteRedirect>,
    )> {
        let mut ctx = Context::unpack(input, Rule::Invocation);

        let exe = ctx.match_rule(Rule::Term);

        // Collect arguments to the function
        let mut arguments = Vec::new();

        // Keep track of the redirects
        let mut read_redirect: Option<ast::ReadRedirect> = None;
        let mut write_redirect: Option<ast::WriteRedirect> = None;

        for pair in ctx.inner() {
            match pair.as_rule() {
                // Parse an argument
                Rule::Term => arguments.push(self.p_term(pair)?),

                // Handle a read redirect
                Rule::ReadRedirect if can_redirect_input => match read_redirect {
                    None => read_redirect = Some(self.p_read_redirect(pair)?),
                    Some(_) => {
                        return Err(make_error(&pair, "found conflicting input redirection"))
                    }
                },
                Rule::ReadRedirect if !can_redirect_input => {
                    return Err(make_error(
                        &pair,
                        "cannot redirect input outside unless it's from the first command in a pipeline",
                    ))
                }

                // Handle a write redirect
                Rule::WriteRedirect if can_redirect_output => match write_redirect {
                    None => write_redirect = Some(self.p_write_redirect(pair)?),
                    Some(_) => {
                        return Err(make_error(&pair, "found conflicting output redirection"))
                    }
                },
                Rule::WriteRedirect if !can_redirect_output => {
                    return Err(make_error(
                        &pair,
                        "cannot redirect output unless it's from the last command in a pipeline",
                    ))
                }

                _ => unreachable!("unexpected rule inside Invocation"),
            }
        }

        ctx.done();

        Ok((
            ast::Invocation {
                executable: self.p_term(exe)?,
                arguments,
            },
            read_redirect,
            write_redirect,
        ))
    }

    fn p_read_redirect(&self, input: Pair<Rule>) -> Result<ast::ReadRedirect> {
        let mut ctx = Context::unpack(input, Rule::ReadRedirect);
        let inner = ctx.match_rule(Rule::RRedirFile);
        ctx.done();

        let redir = match inner.as_rule() {
            Rule::RRedirFile => ast::ReadRedirect::File {
                file: self.p_r_redir_file(inner)?,
            },
            _ => unreachable!("unexpected rule in ReadRedirect"),
        };

        Ok(redir)
    }

    fn p_r_redir_file(&self, input: Pair<Rule>) -> Result<ast::Term> {
        let mut ctx = Context::unpack(input, Rule::RRedirFile);
        let term = ctx.match_rule(Rule::Term);
        ctx.done();
        self.p_term(term)
    }

    fn p_write_redirect(&self, input: Pair<Rule>) -> Result<ast::WriteRedirect> {
        let mut ctx = Context::unpack(input, Rule::WriteRedirect);
        let inner = ctx.match_any();
        ctx.done();

        let redir = match inner.as_rule() {
            Rule::WRedirTruncateFile => ast::WriteRedirect::TruncateFile {
                file: self.p_w_redir_truncate_file(inner)?,
            },
            Rule::WRedirAppendFile => ast::WriteRedirect::AppendFile {
                file: self.p_w_redir_append_file(inner)?,
            },
            _ => unreachable!("unexpected rule in WriteRedirect"),
        };

        Ok(redir)
    }

    fn p_w_redir_truncate_file(&self, input: Pair<Rule>) -> Result<ast::Term> {
        let mut ctx = Context::unpack(input, Rule::WRedirTruncateFile);
        let term = ctx.match_rule(Rule::Term);
        ctx.done();
        self.p_term(term)
    }

    fn p_w_redir_append_file(&self, input: Pair<Rule>) -> Result<ast::Term> {
        let mut ctx = Context::unpack(input, Rule::WRedirAppendFile);
        let term = ctx.match_rule(Rule::Term);
        ctx.done();
        self.p_term(term)
    }

    fn p_term(&self, input: Pair<Rule>) -> Result<ast::Term> {
        let mut ctx = Context::unpack(input, Rule::Term);
        let term = ctx.match_any();
        ctx.done();

        let value = match term.as_rule() {
            Rule::BareTerm => self.p_bare_term(term)?,
            Rule::SingleQuotedStringLiteral => self.p_single_quoted_string_literal(term)?,
            Rule::DoubleQuotedStringLiteral => self.p_double_quoted_string_literal(term)?,
            _ => unreachable!("unexpected flavor of Term"),
        };

        Ok(ast::Term::Literal { value })
    }

    fn p_bare_term(&self, input: Pair<Rule>) -> Result<String> {
        Ok(input.as_str().to_string())
    }

    fn p_single_quoted_string_literal(&self, input: Pair<Rule>) -> Result<String> {
        // Chop off the single-quotes
        let raw = input.as_str();
        Ok(raw[1..raw.len() - 1].to_string())
    }

    fn p_double_quoted_string_literal(&self, input: Pair<Rule>) -> Result<String> {
        // Chop off the double-quotes
        let raw = input.as_str();
        Ok(raw[1..raw.len() - 1].to_string())
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a custom Pest error for the given Pair
fn make_error(failing: &Pair<Rule>, message: impl AsRef<str>) -> Error {
    let kind = pest::error::ErrorVariant::CustomError {
        message: message.as_ref().to_string(),
    };

    pest::error::Error::new_from_span(kind, failing.as_span())
}

/// A context to make dealing with Pest inner Pairs easier
struct Context<'i> {
    inner: Pairs<'i, Rule>,

    #[allow(dead_code)]
    span: Span<'i>,
}

#[allow(dead_code)]
impl<'i> Context<'i> {
    /// Unwrap a Pest [`Pair`] into a [`Context`], asserting it's looking at the right Rule.
    fn unpack(pair: Pair<'i, Rule>, expected: Rule) -> Context<'i> {
        assert_eq!(
            pair.as_rule(),
            expected,
            "Unexpected rule generated by parser"
        );
        Context {
            span: pair.as_span(),
            inner: pair.into_inner(), // unpack
        }
    }

    /// Get the source of the pair used to create this Context.
    fn as_str(&self) -> &'i str {
        self.span.as_str()
    }

    /// Pops the first element of the iterator, asserting it matches the given rule.
    #[track_caller]
    fn match_rule(&mut self, rule: Rule) -> Pair<'i, Rule> {
        let p = self.inner.next().expect("expected a pair to match");
        assert_eq!(p.as_rule(), rule, "next rule did not match");
        p
    }

    /// Only pop the first element of the iterator if it matches the given rule.
    #[track_caller]
    fn match_optional_rule(&mut self, rule: Rule) -> Option<Pair<'i, Rule>> {
        let maybe_p = self.inner.peek();

        maybe_p.and_then(|p| {
            if p.as_rule() == rule {
                self.inner.next()
            } else {
                None
            }
        })
    }

    /// Get all inner elements of the iterator, discarding them.
    fn match_rest(&mut self) -> Vec<Pair<'i, Rule>> {
        self.inner.by_ref().collect()
    }

    /// Pop the first element of the iterator, asserting it matches the given rule, and discarding
    /// it.
    #[track_caller]
    fn match_optional_any(&mut self) -> Option<Pair<'i, Rule>> {
        self.inner.next()
    }

    /// Only pop the first element of the iterator if it exists.
    #[track_caller]
    fn match_any(&mut self) -> Pair<'i, Rule> {
        self.inner
            .next()
            .expect("expected a pair to unconditionally match")
    }

    /// Assert that we've consumed all the contents of this Context.
    #[track_caller]
    fn done(mut self) {
        assert_eq!(self.inner.next(), None, "expected no more pairs");
    }

    /// Mutably borrow this context's wrapped Pest pair iterator
    /// You can consume elements using this method---for instance, by `map`ping a function over
    /// them.
    fn inner(&mut self) -> &mut Pairs<'i, Rule> {
        &mut self.inner
    }
}

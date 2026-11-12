//! Per-pass indexes for queries repeated over a linked program.

use oxc_span::Span;

/// Index literal occurrences, including those in strings/comments. Generated
/// name freshness historically uses substring matching, so an identifier-only
/// set would incorrectly admit names present in comments or longer names.
pub(crate) struct SourceNames<'s> {
    source: &'s str,
    names: Vec<&'s str>,
}

impl<'s> SourceNames<'s> {
    pub fn new(source: &'s str, prefix: &str) -> Self {
        let mut names: Vec<_> = source
            .match_indices(prefix)
            .map(|(start, _)| {
                let tail = &source[start..];
                let end = tail
                    .bytes()
                    .position(|b| !(b.is_ascii_alphanumeric() || b == b'_' || b == b'$'))
                    .unwrap_or(tail.len());
                &tail[..end]
            })
            .collect();
        names.sort_unstable();
        names.dedup();
        Self { source, names }
    }

    /// Generated names use ASCII identifier bytes. Keep the original literal
    /// check for other binding names admitted by the JavaScript parser.
    pub fn contains(&self, name: &str) -> bool {
        if !name.is_ascii() {
            return self.source.contains(name);
        }
        let at = self.names.partition_point(|existing| *existing < name);
        self.names
            .get(at)
            .is_some_and(|existing| existing.starts_with(name))
    }
}

/// AST regions are nested or disjoint. Find the innermost containing region
/// by binary search followed by parent links, rather than scanning every
/// function/block in the program for each reference.
#[derive(Default)]
pub(crate) struct SpanIndex {
    spans: Vec<Span>,
    parents: Vec<Option<usize>>,
}

impl SpanIndex {
    /// Outermost repeated region entered between an original evaluation and
    /// a consumer. Stores anywhere in it may run before a later iteration's
    /// consumer, even when their source text follows that consumer.
    pub fn repeated_between(&self, from: Span, to: Span) -> Option<Span> {
        // The regions holding `to` are its innermost region and that one's
        // ancestors. Once one also holds `from`, so do all outer ones: the
        // answer is the outermost before that point.
        let mut at = self.innermost(to)?;
        let mut outermost = None;
        loop {
            let region = self.spans[at];
            if region.start <= from.start && from.end <= region.end {
                return outermost;
            }
            outermost = Some(region);
            match self.parents[at] {
                Some(parent) => at = parent,
                None => return outermost,
            }
        }
    }

    /// Index of the innermost region holding `span`.
    fn innermost(&self, span: Span) -> Option<usize> {
        let mut at = self
            .spans
            .partition_point(|region| region.start <= span.start)
            .checked_sub(1)?;
        loop {
            let region = self.spans[at];
            if span.end <= region.end {
                return Some(at);
            }
            at = self.parents[at]?;
        }
    }
    pub fn push(&mut self, span: Span) {
        self.spans.push(span);
    }

    pub fn build(&mut self) {
        self.spans
            .sort_by_key(|span| (span.start, std::cmp::Reverse(span.end)));
        self.spans.dedup();
        self.parents.clear();
        let mut stack: Vec<usize> = Vec::new();
        for (index, span) in self.spans.iter().enumerate() {
            while stack
                .last()
                .is_some_and(|&parent| self.spans[parent].end < span.end)
            {
                stack.pop();
            }
            self.parents.push(stack.last().copied());
            stack.push(index);
        }
    }

    pub fn enclosing(&self, span: Span) -> Option<Span> {
        self.innermost(span).map(|at| self.spans[at])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freshness_keeps_substring_checks_including_comments_and_strings() {
        let source = "// $MERGE_12suffix\nconst x = '$MERGE_2'; /* $MERGE_3é */";
        let index = SourceNames::new(source, "$MERGE_");
        for name in [
            "$MERGE_1",
            "$MERGE_12",
            "$MERGE_12suffix",
            "$MERGE_2",
            "$MERGE_3",
            "$MERGE_3é",
            "$MERGE_3éx",
            "$MERGE_4",
        ] {
            assert_eq!(index.contains(name), source.contains(name), "{name}");
        }
    }

    #[test]
    fn repeated_between_matches_linear_scan() {
        let spans = [
            Span::new(0, 100),
            Span::new(5, 20),
            Span::new(10, 15),
            Span::new(25, 40),
            Span::new(26, 39),
            Span::new(50, 80),
            Span::new(50, 70),
        ];
        let mut index = SpanIndex::default();
        for span in spans {
            index.push(span);
        }
        index.build();
        let linear = |from: Span, to: Span| {
            spans
                .iter()
                .copied()
                .filter(|r| r.start <= to.start && to.end <= r.end && !(r.start <= from.start && from.end <= r.end))
                .max_by_key(|r| r.size())
        };
        for a in (0..105).step_by(3) {
            for b in (0..105).step_by(4) {
                let from = Span::new(a, a + 1);
                let to = Span::new(b, b + 1);
                assert_eq!(index.repeated_between(from, to), linear(from, to), "{from:?} {to:?}");
            }
        }
    }

    #[test]
    fn regions_match_linear_lookup_across_nested_and_sibling_spans() {
        let spans = [
            Span::new(0, 100),
            Span::new(5, 20),
            Span::new(10, 15),
            Span::new(25, 40),
            Span::new(50, 80),
            Span::new(50, 70),
        ];
        let mut index = SpanIndex::default();
        for span in spans {
            index.push(span);
        }
        index.build();
        for start in 0..110 {
            for end in start..110 {
                let span = Span::new(start, end);
                let linear = spans
                    .iter()
                    .copied()
                    .filter(|s| s.start <= start && end <= s.end)
                    .min_by_key(|s| s.end - s.start);
                assert_eq!(index.enclosing(span), linear, "{span:?}");
            }
        }
    }
}

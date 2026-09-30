use crate::typed_tree::Node;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub type Path = Vec<usize>;

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    NodeAdded {
        path: Path,
        kind: String,
    },
    NodeRemoved {
        path: Path,
        kind: String,
    },
    NodeModified {
        old_path: Path,
        new_path: Path,
        kind: String,
    },
    AttributeChanged {
        old_path: Path,
        new_path: Path,
        name: String,
        old_value: Option<Value>,
        new_value: Option<Value>,
    },
    TextChanged {
        old_path: Path,
        new_path: Path,
        old_text: Option<String>,
        new_text: Option<String>,
    },
    ChildAdded {
        parent_old_path: Path,
        parent_new_path: Path,
        index: usize,
        kind: String,
    },
    ChildRemoved {
        parent_old_path: Path,
        parent_new_path: Path,
        index: usize,
        kind: String,
    },
}

impl Event {
    pub fn to_value(&self) -> Value {
        match self {
            Self::NodeAdded { path, kind } => {
                json!({"type": "NodeAdded", "path": path, "kind": kind})
            }
            Self::NodeRemoved { path, kind } => {
                json!({"type": "NodeRemoved", "path": path, "kind": kind})
            }
            Self::NodeModified {
                old_path,
                new_path,
                kind,
            } => {
                json!({"type": "NodeModified", "old_path": old_path, "new_path": new_path, "kind": kind})
            }
            Self::AttributeChanged {
                old_path,
                new_path,
                name,
                old_value,
                new_value,
            } => {
                let mut event = json!({
                    "type": "AttributeChanged",
                    "old_path": old_path,
                    "new_path": new_path,
                    "name": name,
                });
                if let Some(v) = old_value {
                    event["old_value"] = v.clone();
                }
                if let Some(v) = new_value {
                    event["new_value"] = v.clone();
                }
                event
            }
            Self::TextChanged {
                old_path,
                new_path,
                old_text,
                new_text,
            } => json!({
                "type": "TextChanged",
                "old_path": old_path,
                "new_path": new_path,
                "old_text": old_text,
                "new_text": new_text,
            }),
            Self::ChildAdded {
                parent_old_path,
                parent_new_path,
                index,
                kind,
            } => json!({
                "type": "ChildAdded",
                "parent_old_path": parent_old_path,
                "parent_new_path": parent_new_path,
                "index": index,
                "kind": kind,
            }),
            Self::ChildRemoved {
                parent_old_path,
                parent_new_path,
                index,
                kind,
            } => json!({
                "type": "ChildRemoved",
                "parent_old_path": parent_old_path,
                "parent_new_path": parent_new_path,
                "index": index,
                "kind": kind,
            }),
        }
    }
}

pub fn diff(old: &Node, new: &Node) -> Vec<Event> {
    let mut events = Vec::new();
    if old.kind != new.kind {
        emit_subtree(old, &mut Vec::new(), &mut events, &|path, kind| {
            Event::NodeRemoved { path, kind }
        });
        emit_subtree(new, &mut Vec::new(), &mut events, &|path, kind| {
            Event::NodeAdded { path, kind }
        });
        return events;
    }
    diff_pair(old, new, &mut Vec::new(), &mut Vec::new(), &mut events);
    events
}

/// `alignment[i]` is the index in `new` that `old[i]` is matched with.
pub type Alignment = Vec<Option<usize>>;

/// Aligns two sibling lists through the exact, structural and text-similarity
/// stages.
pub fn align(old: &[Node], new: &[Node]) -> Alignment {
    let mut alignment: Alignment = vec![None; old.len()];
    let exact = exact_matches(old, new);
    for &(i, j) in &exact {
        alignment[i] = Some(j);
    }
    let mut start = (0, 0);
    for &(i, j) in exact.iter().chain(std::iter::once(&(old.len(), new.len()))) {
        align_gap(old, new, start.0..i, start.1..j, &mut alignment);
        start = (i + 1, j + 1);
    }
    alignment
}

/// Stage 1: longest common subsequence over fingerprints, walked with the
/// documented tie-break.
pub fn exact_matches(old: &[Node], new: &[Node]) -> Vec<(usize, usize)> {
    let (n, m) = (old.len(), new.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if old[i].fingerprint == new[j].fingerprint {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut matches = Vec::new();
    while i < n && j < m {
        if old[i].fingerprint == new[j].fingerprint {
            matches.push((i, j));
            i += 1;
            j += 1;
        } else if lcs[i + 1][j] >= lcs[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    matches
}

fn align_gap(
    old: &[Node],
    new: &[Node],
    old_gap: std::ops::Range<usize>,
    new_gap: std::ops::Range<usize>,
    alignment: &mut Alignment,
) {
    if old_gap.is_empty() || new_gap.is_empty() {
        return;
    }
    if structural_gap(&old[old_gap.clone()], &new[new_gap.clone()]) {
        for (i, j) in old_gap.zip(new_gap) {
            alignment[i] = Some(j);
        }
        return;
    }
    let mut cursor = new_gap.start;
    for i in old_gap {
        if let Some(j) = most_similar(&old[i], new, cursor..new_gap.end) {
            alignment[i] = Some(j);
            cursor = j + 1;
        }
    }
}

/// Stage 2: same length and the same kind at every position.
pub fn structural_gap(old: &[Node], new: &[Node]) -> bool {
    old.len() == new.len() && old.iter().zip(new).all(|(a, b)| a.kind == b.kind)
}

/// Stage 3: the most similar node of the same kind in `range` whose Dice
/// similarity to `node` is at least 1/2; ties go to the lowest index.
pub fn most_similar(node: &Node, new: &[Node], range: std::ops::Range<usize>) -> Option<usize> {
    let grams = Bigrams::of(&node.subtree_text());
    let mut best: Option<(usize, u64, u64)> = None;
    for j in range {
        if new[j].kind != node.kind {
            continue;
        }
        let other = Bigrams::of(&new[j].subtree_text());
        let (shared, total) = (grams.shared(&other), grams.total + other.total);
        if total == 0 || 4 * shared < total {
            continue;
        }
        // shared/total > best_shared/best_total without division
        let better = match best {
            None => true,
            Some((_, bs, bt)) => shared * bt > bs * total,
        };
        if better {
            best = Some((j, shared, total));
        }
    }
    best.map(|(j, _, _)| j)
}

/// Multiset of character bigrams. A one-character text is a single gram.
pub struct Bigrams {
    counts: BTreeMap<(char, Option<char>), u64>,
    total: u64,
}

impl Bigrams {
    pub fn of(text: &str) -> Self {
        let chars: Vec<char> = text.chars().collect();
        let mut counts = BTreeMap::new();
        let mut total = 0;
        match chars.len() {
            0 => {}
            1 => {
                counts.insert((chars[0], None), 1);
                total = 1;
            }
            _ => {
                for pair in chars.windows(2) {
                    *counts.entry((pair[0], Some(pair[1]))).or_insert(0) += 1;
                    total += 1;
                }
            }
        }
        Self { counts, total }
    }

    pub fn shared(&self, other: &Self) -> u64 {
        self.counts
            .iter()
            .map(|(gram, n)| (*n).min(other.counts.get(gram).copied().unwrap_or(0)))
            .sum()
    }

    pub fn total(&self) -> u64 {
        self.total
    }
}

fn diff_pair(
    old: &Node,
    new: &Node,
    old_path: &mut Path,
    new_path: &mut Path,
    out: &mut Vec<Event>,
) {
    if old.fingerprint == new.fingerprint {
        return;
    }
    let text_changed = old.text != new.text;
    let attributes = changed_attributes(old, new);
    if text_changed || !attributes.is_empty() {
        out.push(Event::NodeModified {
            old_path: old_path.clone(),
            new_path: new_path.clone(),
            kind: new.kind.clone(),
        });
        for (name, old_value, new_value) in attributes {
            out.push(Event::AttributeChanged {
                old_path: old_path.clone(),
                new_path: new_path.clone(),
                name,
                old_value,
                new_value,
            });
        }
        if text_changed {
            out.push(Event::TextChanged {
                old_path: old_path.clone(),
                new_path: new_path.clone(),
                old_text: old.text.clone(),
                new_text: new.text.clone(),
            });
        }
    }

    let alignment = align(&old.children, &new.children);
    let mut new_matched = vec![false; new.children.len()];
    for j in alignment.iter().flatten() {
        new_matched[*j] = true;
    }
    let (mut i, mut j) = (0, 0);
    while i < old.children.len() || j < new.children.len() {
        if i < old.children.len() && alignment[i].is_none() {
            let child = &old.children[i];
            out.push(Event::ChildRemoved {
                parent_old_path: old_path.clone(),
                parent_new_path: new_path.clone(),
                index: i,
                kind: child.kind.clone(),
            });
            old_path.push(i);
            emit_subtree(child, old_path, out, &|path, kind| Event::NodeRemoved {
                path,
                kind,
            });
            old_path.pop();
            i += 1;
        } else if j < new.children.len() && !new_matched[j] {
            let child = &new.children[j];
            out.push(Event::ChildAdded {
                parent_old_path: old_path.clone(),
                parent_new_path: new_path.clone(),
                index: j,
                kind: child.kind.clone(),
            });
            new_path.push(j);
            emit_subtree(child, new_path, out, &|path, kind| Event::NodeAdded {
                path,
                kind,
            });
            new_path.pop();
            j += 1;
        } else {
            old_path.push(i);
            new_path.push(j);
            diff_pair(&old.children[i], &new.children[j], old_path, new_path, out);
            old_path.pop();
            new_path.pop();
            i += 1;
            j += 1;
        }
    }
}

type AttributeChange = (String, Option<Value>, Option<Value>);

fn changed_attributes(old: &Node, new: &Node) -> Vec<AttributeChange> {
    let (a, b) = (&old.attributes, &new.attributes);
    let mut changes = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        let order = match (a.get(i), b.get(j)) {
            (Some(x), Some(y)) => crate::canonical::cmp_utf16(&x.0, &y.0),
            (Some(_), None) => std::cmp::Ordering::Less,
            _ => std::cmp::Ordering::Greater,
        };
        match order {
            std::cmp::Ordering::Less => {
                changes.push((a[i].0.clone(), Some(a[i].1.clone()), None));
                i += 1;
            }
            std::cmp::Ordering::Greater => {
                changes.push((b[j].0.clone(), None, Some(b[j].1.clone())));
                j += 1;
            }
            std::cmp::Ordering::Equal => {
                if a[i].1 != b[j].1 {
                    changes.push((a[i].0.clone(), Some(a[i].1.clone()), Some(b[j].1.clone())));
                }
                i += 1;
                j += 1;
            }
        }
    }
    changes
}

fn emit_subtree(
    node: &Node,
    path: &mut Path,
    out: &mut Vec<Event>,
    make: &dyn Fn(Path, String) -> Event,
) {
    out.push(make(path.clone(), node.kind.clone()));
    for (i, child) in node.children.iter().enumerate() {
        path.push(i);
        emit_subtree(child, path, out, make);
        path.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(kind: &str, text: Option<&str>, children: Vec<Value>) -> Value {
        let mut v = json!({"kind": kind, "children": children});
        if let Some(t) = text {
            v["text"] = json!(t);
        }
        v
    }

    fn p(text: &str) -> Value {
        n("p", Some(text), vec![])
    }

    fn nodes(values: Vec<Value>) -> Vec<Node> {
        values
            .iter()
            .map(|v| Node::from_value(v, "x").unwrap())
            .collect()
    }

    #[test]
    fn exact_stage_uses_the_documented_tie_break() {
        let old = nodes(vec![p("a"), p("b")]);
        let new = nodes(vec![p("b"), p("a")]);
        assert_eq!(exact_matches(&old, &new), vec![(1, 0)]);
        assert_eq!(align(&old, &new), vec![None, Some(0)]);
    }

    #[test]
    fn structural_stage_pairs_equal_length_gaps_by_position() {
        let old = nodes(vec![p("keep"), p("alpha"), p("beta"), p("tail")]);
        let new = nodes(vec![p("keep"), p("zzzz"), p("yyyy"), p("tail")]);
        assert_eq!(align(&old, &new), vec![Some(0), Some(1), Some(2), Some(3)]);
    }

    #[test]
    fn equal_length_gap_with_different_kinds_falls_through_to_similarity() {
        let old = nodes(vec![n("h", Some("title text"), vec![]), p("body text")]);
        let new = nodes(vec![p("body text!"), n("h", Some("title text?"), vec![])]);
        assert_eq!(align(&old, &new), vec![Some(1), None]);
    }

    #[test]
    fn similarity_threshold_is_one_half() {
        let at_half = nodes(vec![p("xyz0"), p("abd")]);
        let below = nodes(vec![p("xyz0"), p("abde")]);
        let old = nodes(vec![p("abc")]);
        assert_eq!(align(&old, &at_half), vec![Some(1)]);
        assert_eq!(align(&old, &below), vec![None]);
    }

    #[test]
    fn similarity_ties_go_to_the_lowest_index_and_the_cursor_advances() {
        let old = nodes(vec![p("abcd")]);
        let new = nodes(vec![p("abcdX"), p("abcdX")]);
        assert_eq!(align(&old, &new), vec![Some(0)]);

        let old = nodes(vec![p("abcdef"), p("uvwxyz")]);
        let new = nodes(vec![p("xxxxx"), p("uvwxyz!"), p("abcdef!")]);
        assert_eq!(align(&old, &new), vec![Some(2), None]);
    }

    #[test]
    fn bigram_edge_cases() {
        assert_eq!(Bigrams::of("").total(), 0);
        assert_eq!(Bigrams::of("a").total(), 1);
        assert_eq!(Bigrams::of("aaa").total(), 2);
        assert_eq!(Bigrams::of("aaa").shared(&Bigrams::of("aa")), 1);
        let old = nodes(vec![json!({"kind": "p", "attributes": {"a": 1}})]);
        let new = nodes(vec![n("p", None, vec![]), p("x")]);
        assert_eq!(align(&old, &new), vec![None]);
    }

    #[test]
    fn similarity_reads_descendant_text() {
        let old = nodes(vec![n("s", None, vec![p("quick brown fox")])]);
        let new = nodes(vec![p("zzzzzz"), n("s", None, vec![p("quick brown fax")])]);
        assert_eq!(most_similar(&old[0], &new, 0..2), Some(1));
    }

    #[test]
    fn diff_is_deterministic() {
        let a = Node::from_value(&n("d", None, vec![p("one"), p("two")]), "old").unwrap();
        let b = Node::from_value(&n("d", None, vec![p("two"), p("three")]), "new").unwrap();
        assert_eq!(diff(&a, &b), diff(&a, &b));
    }
}

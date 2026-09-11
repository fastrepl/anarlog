//! `ContentMatch` (`prosemirror-model/src/content.ts`): a content
//! expression parsed into an NFA and compiled to a DFA whose states answer
//! `matchType`, `fillBefore` and `findWrapping`.

use std::collections::{HashMap, VecDeque};

use super::node::{Fragment, Node};
use super::schema::{Schema, TypeId};

#[derive(Debug, Default)]
pub struct Dfa {
    states: Vec<State>,
}

#[derive(Debug, Default)]
struct State {
    valid_end: bool,
    next: Vec<(TypeId, usize)>,
}

/// A position in a content match: a DFA and the state reached.
#[derive(Clone, Copy)]
pub struct Match {
    pub dfa: &'static Dfa,
    pub state: usize,
}

#[derive(Debug, Clone)]
enum Expr {
    Choice(Vec<Expr>),
    Seq(Vec<Expr>),
    Plus(Box<Expr>),
    Star(Box<Expr>),
    Opt(Box<Expr>),
    Range(usize, Option<usize>, Box<Expr>),
    Name(TypeId),
}

#[derive(Debug, Clone, Copy)]
struct Edge {
    term: Option<TypeId>,
    to: Option<usize>,
}

impl Dfa {
    /// `ContentMatch.parse`: an empty expression is `ContentMatch.empty`.
    pub fn parse(expr: &str, resolve: &dyn Fn(&str) -> Vec<TypeId>, inline: &[bool]) -> Dfa {
        let tokens = tokenize(expr);
        if tokens.is_empty() {
            return Dfa {
                states: vec![State {
                    valid_end: true,
                    next: Vec::new(),
                }],
            };
        }
        let mut stream = TokenStream {
            tokens,
            pos: 0,
            resolve,
            inline,
            inline_seen: None,
        };
        let expr = parse_expr(&mut stream);
        assert!(
            stream.next().is_none(),
            "unexpected trailing text in content expression '{expr:?}'"
        );
        dfa(&nfa(&expr))
    }

    /// `ContentMatch.empty`: no edges, a valid end.
    pub fn is_empty(&self) -> bool {
        self.states.len() == 1 && self.states[0].next.is_empty()
    }

    pub fn first_type(&self, state: usize) -> Option<TypeId> {
        self.states[state].next.first().map(|(id, _)| *id)
    }

    pub fn valid_end(&self, state: usize) -> bool {
        self.states[state].valid_end
    }

    pub fn match_type(&self, state: usize, id: TypeId) -> Option<usize> {
        self.states[state]
            .next
            .iter()
            .find(|(next, _)| *next == id)
            .map(|(_, to)| *to)
    }

    pub fn edges(&self, state: usize) -> &[(TypeId, usize)] {
        &self.states[state].next
    }

    /// `compatible`: the two states share an edge type.
    pub fn compatible(&self, state: usize, other: &Dfa, other_state: usize) -> bool {
        self.states[state].next.iter().any(|(id, _)| {
            other.states[other_state]
                .next
                .iter()
                .any(|(other_id, _)| other_id == id)
        })
    }
}

impl Match {
    pub fn valid_end(&self) -> bool {
        self.dfa.valid_end(self.state)
    }

    pub fn match_type(&self, id: TypeId) -> Option<Match> {
        self.dfa.match_type(self.state, id).map(|state| Match {
            dfa: self.dfa,
            state,
        })
    }

    pub fn match_fragment(&self, fragment: &Fragment, start: usize, end: usize) -> Option<Match> {
        let mut cur = *self;
        for child in &fragment.children[start..end] {
            cur = cur.match_type(child.type_id)?;
        }
        Some(cur)
    }

    pub fn edges(&self) -> &'static [(TypeId, usize)] {
        self.dfa.edges(self.state)
    }

    /// `defaultType`: the first edge type that can be created from nothing.
    pub fn default_type(&self, schema: &Schema) -> Option<TypeId> {
        self.edges()
            .iter()
            .map(|(id, _)| *id)
            .find(|id| !(schema.nodes[*id].is_text() || schema.nodes[*id].has_required_attrs()))
    }

    /// `fillBefore`: the shortest sequence of generatable nodes after which
    /// `after` (from `start_index`) matches, ending validly when `to_end`.
    pub fn fill_before(
        &self,
        after: &Fragment,
        to_end: bool,
        start_index: usize,
        schema: &'static Schema,
    ) -> Option<Fragment> {
        let mut seen: Vec<usize> = vec![self.state];
        fn search(
            m: Match,
            types: Vec<TypeId>,
            after: &Fragment,
            to_end: bool,
            start_index: usize,
            schema: &'static Schema,
            seen: &mut Vec<usize>,
        ) -> Option<Fragment> {
            if let Some(finished) = m.match_fragment(after, start_index, after.children.len())
                && (!to_end || finished.valid_end())
            {
                return Some(Fragment::from(
                    types
                        .iter()
                        .map(|id| Node::create_and_fill(schema, *id, None, None, Vec::new()))
                        .collect::<Option<Vec<_>>>()?,
                ));
            }
            for (id, next) in m.edges() {
                let node = &schema.nodes[*id];
                if !(node.is_text() || node.has_required_attrs() || seen.contains(next)) {
                    seen.push(*next);
                    let mut with = types.clone();
                    with.push(*id);
                    if let Some(found) = search(
                        Match {
                            dfa: m.dfa,
                            state: *next,
                        },
                        with,
                        after,
                        to_end,
                        start_index,
                        schema,
                        seen,
                    ) {
                        return Some(found);
                    }
                }
            }
            None
        }
        search(
            *self,
            Vec::new(),
            after,
            to_end,
            start_index,
            schema,
            &mut seen,
        )
    }

    /// `findWrapping`: the node types to wrap `target` in so it fits here,
    /// found breadth-first; empty when it fits directly.
    pub fn find_wrapping(&self, target: TypeId, schema: &'static Schema) -> Option<Vec<TypeId>> {
        struct Active {
            m: Match,
            ty: Option<TypeId>,
            via: Option<usize>,
        }
        let mut seen: Vec<TypeId> = Vec::new();
        let mut all: Vec<Active> = vec![Active {
            m: *self,
            ty: None,
            via: None,
        }];
        let mut queue: VecDeque<usize> = VecDeque::from([0]);
        while let Some(current) = queue.pop_front() {
            if all[current].m.match_type(target).is_some() {
                let mut result = Vec::new();
                let mut obj = current;
                while let Some(ty) = all[obj].ty {
                    result.push(ty);
                    obj = all[obj].via.expect("a wrapper has a source");
                }
                result.reverse();
                return Some(result);
            }
            let edges: Vec<(TypeId, usize)> = all[current].m.edges().to_vec();
            for (id, next) in edges {
                let node = &schema.nodes[id];
                let current_ty = all[current].ty;
                if !node.is_leaf()
                    && !node.has_required_attrs()
                    && !seen.contains(&id)
                    && (current_ty.is_none() || all[current].m.dfa.valid_end(next))
                {
                    all.push(Active {
                        m: Match {
                            dfa: &node.dfa,
                            state: 0,
                        },
                        ty: Some(id),
                        via: Some(current),
                    });
                    queue.push_back(all.len() - 1);
                    seen.push(id);
                }
            }
        }
        None
    }
}

struct TokenStream<'a> {
    tokens: Vec<String>,
    pos: usize,
    resolve: &'a dyn Fn(&str) -> Vec<TypeId>,
    inline: &'a [bool],
    inline_seen: Option<bool>,
}

impl TokenStream<'_> {
    fn next(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(String::as_str)
    }

    fn eat(&mut self, tok: &str) -> bool {
        if self.next() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
}

/// `string.split(/\s*(?=\b|\W|$)/)`: words, single punctuation characters.
fn tokenize(expr: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for ch in expr.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            word.push(ch);
            continue;
        }
        if !word.is_empty() {
            tokens.push(std::mem::take(&mut word));
        }
        if !ch.is_whitespace() {
            tokens.push(ch.to_string());
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}

fn parse_expr(stream: &mut TokenStream) -> Expr {
    let mut exprs = vec![parse_expr_seq(stream)];
    while stream.eat("|") {
        exprs.push(parse_expr_seq(stream));
    }
    if exprs.len() == 1 {
        exprs.remove(0)
    } else {
        Expr::Choice(exprs)
    }
}

fn parse_expr_seq(stream: &mut TokenStream) -> Expr {
    let mut exprs = vec![parse_expr_subscript(stream)];
    while stream.next().is_some_and(|next| next != ")" && next != "|") {
        exprs.push(parse_expr_subscript(stream));
    }
    if exprs.len() == 1 {
        exprs.remove(0)
    } else {
        Expr::Seq(exprs)
    }
}

fn parse_expr_subscript(stream: &mut TokenStream) -> Expr {
    let mut expr = parse_expr_atom(stream);
    loop {
        if stream.eat("+") {
            expr = Expr::Plus(Box::new(expr));
        } else if stream.eat("*") {
            expr = Expr::Star(Box::new(expr));
        } else if stream.eat("?") {
            expr = Expr::Opt(Box::new(expr));
        } else if stream.eat("{") {
            expr = parse_expr_range(stream, expr);
        } else {
            break;
        }
    }
    expr
}

fn parse_num(stream: &mut TokenStream) -> usize {
    let value = stream
        .next()
        .and_then(|next| next.parse().ok())
        .expect("a number in the content expression");
    stream.pos += 1;
    value
}

fn parse_expr_range(stream: &mut TokenStream, expr: Expr) -> Expr {
    let min = parse_num(stream);
    let mut max = Some(min);
    if stream.eat(",") {
        max = if stream.next() != Some("}") {
            Some(parse_num(stream))
        } else {
            None
        };
    }
    assert!(stream.eat("}"), "unclosed braced range");
    Expr::Range(min, max, Box::new(expr))
}

fn parse_expr_atom(stream: &mut TokenStream) -> Expr {
    if stream.eat("(") {
        let expr = parse_expr(stream);
        assert!(stream.eat(")"), "missing closing paren");
        return expr;
    }
    let name = stream.next().expect("a token").to_string();
    assert!(
        name.chars().all(|c| c.is_alphanumeric() || c == '_'),
        "unexpected token '{name}'"
    );
    let types = (stream.resolve)(&name);
    assert!(!types.is_empty(), "no node type or group '{name}'");
    for id in &types {
        let inline = stream.inline[*id];
        match stream.inline_seen {
            None => stream.inline_seen = Some(inline),
            Some(seen) => assert_eq!(seen, inline, "mixing inline and block content"),
        }
    }
    stream.pos += 1;
    if types.len() == 1 {
        Expr::Name(types[0])
    } else {
        Expr::Choice(types.into_iter().map(Expr::Name).collect())
    }
}

struct Nfa {
    nodes: Vec<Vec<Edge>>,
}

fn nfa(expr: &Expr) -> Nfa {
    let mut nfa = Nfa {
        nodes: vec![Vec::new()],
    };
    let edges = compile(&mut nfa, expr, 0);
    let end = node(&mut nfa);
    connect(&mut nfa, &edges, end);
    nfa
}

fn node(nfa: &mut Nfa) -> usize {
    nfa.nodes.push(Vec::new());
    nfa.nodes.len() - 1
}

/// An edge is addressed by (node, index) so `connect` can set its target.
fn edge(nfa: &mut Nfa, from: usize, to: Option<usize>, term: Option<TypeId>) -> (usize, usize) {
    nfa.nodes[from].push(Edge { term, to });
    (from, nfa.nodes[from].len() - 1)
}

fn connect(nfa: &mut Nfa, edges: &[(usize, usize)], to: usize) {
    for (from, index) in edges {
        nfa.nodes[*from][*index].to = Some(to);
    }
}

fn compile(nfa: &mut Nfa, expr: &Expr, from: usize) -> Vec<(usize, usize)> {
    match expr {
        Expr::Choice(exprs) => exprs
            .iter()
            .flat_map(|expr| compile(nfa, expr, from))
            .collect(),
        Expr::Seq(exprs) => {
            let mut from = from;
            for (i, expr) in exprs.iter().enumerate() {
                let next = compile(nfa, expr, from);
                if i == exprs.len() - 1 {
                    return next;
                }
                from = node(nfa);
                connect(nfa, &next, from);
            }
            unreachable!()
        }
        Expr::Star(inner) => {
            let loop_node = node(nfa);
            edge(nfa, from, Some(loop_node), None);
            let inner_edges = compile(nfa, inner, loop_node);
            connect(nfa, &inner_edges, loop_node);
            vec![edge(nfa, loop_node, None, None)]
        }
        Expr::Plus(inner) => {
            let loop_node = node(nfa);
            let first = compile(nfa, inner, from);
            connect(nfa, &first, loop_node);
            let again = compile(nfa, inner, loop_node);
            connect(nfa, &again, loop_node);
            vec![edge(nfa, loop_node, None, None)]
        }
        Expr::Opt(inner) => {
            let mut edges = vec![edge(nfa, from, None, None)];
            edges.extend(compile(nfa, inner, from));
            edges
        }
        Expr::Range(min, max, inner) => {
            let mut cur = from;
            for _ in 0..*min {
                let next = node(nfa);
                let edges = compile(nfa, inner, cur);
                connect(nfa, &edges, next);
                cur = next;
            }
            match max {
                None => {
                    let edges = compile(nfa, inner, cur);
                    connect(nfa, &edges, cur);
                }
                Some(max) => {
                    for _ in *min..*max {
                        let next = node(nfa);
                        edge(nfa, cur, Some(next), None);
                        let edges = compile(nfa, inner, cur);
                        connect(nfa, &edges, next);
                        cur = next;
                    }
                }
            }
            vec![edge(nfa, cur, None, None)]
        }
        Expr::Name(id) => vec![edge(nfa, from, None, Some(*id))],
    }
}

fn null_from(nfa: &Nfa, node: usize) -> Vec<usize> {
    fn scan(nfa: &Nfa, node: usize, result: &mut Vec<usize>) {
        let edges = &nfa.nodes[node];
        if edges.len() == 1 && edges[0].term.is_none() {
            return scan(nfa, edges[0].to.unwrap(), result);
        }
        result.push(node);
        for edge in edges {
            if edge.term.is_none()
                && let Some(to) = edge.to
                && !result.contains(&to)
            {
                scan(nfa, to, result);
            }
        }
    }
    let mut result = Vec::new();
    scan(nfa, node, &mut result);
    result.sort_by(|a, b| b.cmp(a));
    result
}

fn dfa(nfa: &Nfa) -> Dfa {
    let mut dfa = Dfa::default();
    let mut labeled: HashMap<Vec<usize>, usize> = HashMap::new();
    let end = nfa.nodes.len() - 1;
    explore(nfa, &mut dfa, &mut labeled, null_from(nfa, 0), end);
    dfa
}

fn explore(
    nfa: &Nfa,
    dfa: &mut Dfa,
    labeled: &mut HashMap<Vec<usize>, usize>,
    states: Vec<usize>,
    end: usize,
) -> usize {
    let mut out: Vec<(TypeId, Vec<usize>)> = Vec::new();
    for node in &states {
        for edge in &nfa.nodes[*node] {
            let Some(term) = edge.term else {
                continue;
            };
            let targets = null_from(nfa, edge.to.unwrap());
            let set = match out.iter().position(|(t, _)| *t == term) {
                Some(index) => index,
                None => {
                    out.push((term, Vec::new()));
                    out.len() - 1
                }
            };
            for target in targets {
                if !out[set].1.contains(&target) {
                    out[set].1.push(target);
                }
            }
        }
    }
    dfa.states.push(State {
        valid_end: states.contains(&end),
        next: Vec::new(),
    });
    let state = dfa.states.len() - 1;
    labeled.insert(states, state);
    for (term, mut targets) in out {
        targets.sort_by(|a, b| b.cmp(a));
        let next = match labeled.get(&targets) {
            Some(next) => *next,
            None => explore(nfa, dfa, labeled, targets, end),
        };
        dfa.states[state].next.push((term, next));
    }
    state
}

#[cfg(test)]
mod tests {
    use super::super::schema::schema;
    use super::*;

    fn m(name: &str) -> Match {
        let s = schema();
        Match {
            dfa: &s.nodes[s.node(name).unwrap()].dfa,
            state: 0,
        }
    }

    #[test]
    fn matches_follow_the_expressions() {
        let s = schema();
        let (p, li, ul, text, hr) = (
            s.node("paragraph").unwrap(),
            s.node("listItem").unwrap(),
            s.node("bulletList").unwrap(),
            s.text_type(),
            s.node("horizontalRule").unwrap(),
        );
        // `block+`
        let doc = m("doc");
        assert!(!doc.valid_end());
        assert!(doc.match_type(p).unwrap().valid_end());
        assert!(doc.match_type(li).is_none());
        // `paragraph block*`
        let item = m("listItem");
        assert!(item.match_type(ul).is_none());
        let after_p = item.match_type(p).unwrap();
        assert!(after_p.valid_end());
        assert!(after_p.match_type(ul).is_some());
        assert!(after_p.match_type(p).is_some());
        // `inline*`
        let para = m("paragraph");
        assert!(para.valid_end());
        assert!(para.match_type(text).is_some());
        assert!(para.match_type(p).is_none());
        // `(tableCell | tableHeader)*`
        let row = m("tableRow");
        assert!(row.match_type(s.node("tableHeader").unwrap()).is_some());
        assert!(row.match_type(hr).is_none());
        assert!(m("horizontalRule").dfa.is_empty());
        assert_eq!(doc.default_type(s), Some(p));
        assert_eq!(para.default_type(s), Some(s.node("hardBreak").unwrap()));
    }

    #[test]
    fn wrapping_and_filling_follow_prosemirror() {
        let s = schema();
        let (p, li, ul, text) = (
            s.node("paragraph").unwrap(),
            s.node("listItem").unwrap(),
            s.node("bulletList").unwrap(),
            s.text_type(),
        );
        assert_eq!(m("doc").find_wrapping(text, s), Some(vec![p]));
        assert_eq!(m("doc").find_wrapping(li, s), Some(vec![ul]));
        assert_eq!(m("doc").find_wrapping(p, s), Some(vec![]));
        assert_eq!(m("paragraph").find_wrapping(p, s), None);
        // A list item needs its paragraph before a nested list.
        let fill = m("listItem")
            .fill_before(
                &Fragment::from(vec![Node::new(
                    s,
                    ul,
                    None,
                    Fragment::default(),
                    Vec::new(),
                )]),
                false,
                0,
                s,
            )
            .unwrap();
        assert_eq!(fill.children.len(), 1);
        assert_eq!(fill.children[0].type_id, p);
        // `block+` at the end: one paragraph to be valid.
        let fill = m("doc")
            .fill_before(&Fragment::default(), true, 0, s)
            .unwrap();
        assert_eq!(fill.children.len(), 1);
        assert_eq!(fill.children[0].type_id, p);
        // Nothing needed when already valid.
        let fill = m("paragraph")
            .fill_before(&Fragment::default(), true, 0, s)
            .unwrap();
        assert!(fill.children.is_empty());
    }
}

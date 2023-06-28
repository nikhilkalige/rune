#![allow(dead_code)]

use smallvec::{smallvec, SmallVec};
use std::{
    fmt::Display,
    iter::Sum,
    ops::{Add, AddAssign, Sub, SubAssign},
    ptr::NonNull,
};

const MAX: usize = 4;

type Metrics = SmallVec<[Metric; MAX]>;
type IntChildren = SmallVec<[Box<Internal>; MAX]>;
type LeafChildren = SmallVec<[Box<Leaf>; MAX]>;

pub(crate) fn new_root() -> Box<Internal> {
    let mut root = Box::new(Internal::new_leaf());
    root.metrics.push(Metric::default());
    let root_ptr = NonNull::from(&*root);
    if let Node::Leaf(children) = &mut root.children {
        children.push(Box::new(Leaf::new(root_ptr)));
    }
    root
}

#[derive(Debug)]
enum Node {
    Internal(IntChildren),
    Leaf(LeafChildren),
}

impl Node {
    fn len(&self) -> usize {
        match self {
            Node::Internal(x) => x.len(),
            Node::Leaf(x) => x.len(),
        }
    }

    fn set_parent(&mut self, children: NonNull<Internal>) {
        match self {
            Node::Internal(x) => {
                for child in x.iter_mut() {
                    child.parent = Some(children);
                }
            }
            Node::Leaf(x) => {
                for child in x.iter_mut() {
                    child.parent = Some(children);
                }
            }
        }
    }

    fn assert_parent(&self, parent: NonNull<Internal>) {
        match self {
            Node::Internal(x) => {
                for child in x.iter() {
                    assert_eq!(child.parent, Some(parent));
                }
            }
            Node::Leaf(x) => {
                for child in x.iter() {
                    assert_eq!(child.parent, Some(parent));
                }
            }
        }
    }
}

#[derive(Debug)]
struct Internal {
    metrics: Metrics,
    children: Node,
    parent: Option<NonNull<Internal>>,
}

impl Internal {
    fn new_leaf() -> Self {
        Self {
            metrics: SmallVec::new(),
            children: Node::Leaf(SmallVec::new()),
            parent: None,
        }
    }

    fn new_internal() -> Self {
        Self {
            metrics: SmallVec::new(),
            children: Node::Internal(SmallVec::new()),
            parent: None,
        }
    }

    fn clear_parent(mut self: Box<Self>) -> Box<Self> {
        self.parent = None;
        self
    }

    fn metrics(&self) -> Metric {
        self.metrics.iter().copied().sum()
    }

    fn len(&self) -> usize {
        self.children.len()
    }

    fn insert_internal(
        children: &mut IntChildren,
        metrics: &mut Metrics,
        idx: usize,
        new_child: Box<Internal>,
    ) -> Option<Box<Internal>> {
        let len = children.len();
        // update the metrics for the current child
        metrics[idx] = children[idx].metrics();
        // shift idx to the right
        let idx = idx + 1;
        if len < MAX {
            // If there is room in this node then insert the
            // node before the current one
            metrics.insert(idx, new_child.metrics());
            children.insert(idx, new_child);
            None
        } else {
            assert_eq!(len, MAX);
            // split this node into two and return the left one
            let middle = MAX / 2;

            let mut right_metrics: Metrics = metrics.drain(middle..).collect();
            let mut right_children: IntChildren =
                children.drain(middle..).map(|x| x.clear_parent()).collect();
            if idx < middle {
                metrics.insert(idx, new_child.metrics());
                children.insert(idx, new_child);
            } else {
                right_metrics.insert(idx - middle, new_child.metrics());
                right_children.insert(idx - middle, new_child);
            }
            let right = Internal {
                metrics: right_metrics,
                children: Node::Internal(right_children),
                parent: None,
            };
            // Box it so it has a stable address
            let mut boxed = Box::new(right);
            let child_parent = NonNull::from(&*boxed);
            boxed.children.set_parent(child_parent);
            Some(boxed)
        }
    }

    fn insert_leaf(
        children: &mut LeafChildren,
        metrics: &mut Metrics,
        self_ptr: NonNull<Internal>,
        idx: usize,
        needle: Metric,
    ) -> Option<Box<Internal>> {
        let len = children.len();
        let new_metric = metrics[idx] - needle;
        metrics[idx] = needle;
        // shift idx to the right
        let idx = idx + 1;
        if len < MAX {
            // If there is room in this node then insert the
            // leaf before the current one, splitting the
            // size
            metrics.insert(idx, new_metric);
            children.insert(idx, Box::new(Leaf::new(self_ptr)));
            None
        } else {
            assert_eq!(len, MAX);
            // split this node into two and return the left one
            let middle = MAX / 2;
            let mut right_metrics: Metrics = metrics.drain(middle..).collect();
            let mut right_children: LeafChildren =
                children.drain(middle..).map(|x| x.clear_parent()).collect();
            let mut new = Leaf::default();
            if idx < middle {
                new.parent = Some(self_ptr);
                metrics.insert(idx, new_metric);
                children.insert(idx, Box::new(new));
            } else {
                right_metrics.insert(idx - middle, new_metric);
                right_children.insert(idx - middle, Box::new(new));
            }
            let right = Internal {
                metrics: right_metrics,
                children: Node::Leaf(right_children),
                parent: None,
            };
            let mut boxed = Box::new(right);
            // update the children's parent pointer
            let child_parent = NonNull::from(&*boxed);
            boxed.children.set_parent(child_parent);
            Some(boxed)
        }
    }

    fn push_leaf(
        children: &mut LeafChildren,
        metrics: &mut Metrics,
        self_ptr: NonNull<Internal>,
        metric: Metric,
    ) -> Option<Box<Internal>> {
        let len = children.len();
        if len < MAX {
            // If there is room in this node then insert the
            // leaf before the current one, splitting the
            // size
            metrics.push(metric);
            children.push(Box::new(Leaf::new(self_ptr)));
            None
        } else {
            assert_eq!(len, MAX);
            // split this node into two and return the left one
            let new = Leaf::default();
            let right_metrics: Metrics = smallvec![metric];
            let right_children: LeafChildren = smallvec![Box::new(new)];
            let right = Internal {
                metrics: right_metrics,
                children: Node::Leaf(right_children),
                parent: None,
            };
            let mut boxed = Box::new(right);
            // update the children's parent pointer
            let child_parent = NonNull::from(&*boxed);
            boxed.children.set_parent(child_parent);
            Some(boxed)
        }
    }

    pub(crate) fn search_byte(&self, bytes: usize) -> Metric {
        self.search_impl::<{ Self::BYTE }>(bytes)
    }

    pub(crate) fn search_char(&self, chars: usize) -> Metric {
        self.search_impl::<{ Self::CHAR }>(chars)
    }

    const BYTE: u8 = 0;
    const CHAR: u8 = 1;

    fn search_impl<const TYPE: u8>(&self, needle: usize) -> Metric {
        let mut needle = needle;
        let mut sum = Metric::default();
        for (idx, metric) in self.metrics.iter().enumerate() {
            // fast path if we happen get the exact position in the node
            if needle == 0 {
                break;
            }
            let pos = match TYPE {
                Self::BYTE => metric.bytes,
                Self::CHAR => metric.chars,
                _ => unreachable!(),
            };
            if needle < pos {
                let child_sum = match &self.children {
                    Node::Internal(children) => sum + children[idx].search_impl::<TYPE>(needle),
                    Node::Leaf(_) => sum,
                };
                return child_sum;
            }
            sum += *metric;
            needle -= pos;
        }
        sum
    }

    pub(crate) fn insert(self: &mut Box<Self>, needle: Metric) {
        match self.insert_impl(needle) {
            None => {}
            Some(right) => {
                // split the root, making the old root the right child
                let left = std::mem::replace(self, Box::new(Internal::new_internal()));
                self.metrics = smallvec![left.metrics(), right.metrics()];
                self.children = Node::Internal(smallvec![left, right]);
                let this = NonNull::from(&**self);
                self.children.set_parent(this);
            }
        }
    }

    fn insert_impl(&mut self, mut needle: Metric) -> Option<Box<Internal>> {
        self.assert_invariants();
        let self_ptr = NonNull::from(&*self);
        let last = self.metrics.len() - 1;
        for (idx, metric) in self.metrics.iter_mut().enumerate() {
            let in_range = needle.chars < metric.chars;
            if idx == last || in_range {
                let mut new = match &mut self.children {
                    // call recursively and insert the new node
                    Node::Internal(children) => match children[idx].insert_impl(needle) {
                        Some(new) => Self::insert_internal(children, &mut self.metrics, idx, new),
                        None => {
                            // update the metric of the current node because we
                            // increased the max size
                            if !in_range {
                                assert_eq!(idx, last);
                                *metric = children.last().unwrap().metrics();
                            }
                            None
                        }
                    },
                    Node::Leaf(children) => {
                        if in_range {
                            Self::insert_leaf(children, &mut self.metrics, self_ptr, idx, needle)
                        } else {
                            assert_eq!(idx, last);
                            needle -= *metric;
                            Self::push_leaf(children, &mut self.metrics, self_ptr, needle)
                        }
                    }
                };
                // set the parent pointer of the new node
                if let Some(new) = &mut new {
                    new.parent = self.parent;
                }
                return new;
            } else {
                needle -= *metric;
            }
        }
        unreachable!("we should always recurse into a child node");
    }

    fn insert_child(
        &mut self,
        idx: usize,
        this: NonNull<Self>,
        needle: Metric,
    ) -> Option<Box<Self>> {
        let metrics = &mut self.metrics;
        let mut new = match &mut self.children {
            // call recursively and insert the new node
            Node::Internal(children) => match children[idx].insert_impl(needle) {
                Some(new) => Self::insert_internal(children, metrics, idx, new),
                None => None,
            },
            Node::Leaf(children) => Self::insert_leaf(children, metrics, this, idx, needle),
        };
        // set the parent pointer of the new node
        if let Some(new) = &mut new {
            new.parent = self.parent;
        }
        new
    }

    fn assert_invariants(&self) {
        if let Node::Internal(x) = &self.children {
            let metrics = x.iter().map(|x| x.metrics()).sum();
            assert_eq!(self.metrics(), metrics);
        };
        let this = NonNull::from(self);
        self.children.assert_parent(this);
    }
}

impl Display for Internal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // print the children level by level by adding them to a pair of
        // alternating arrays for each level
        let mut current = Vec::new();
        let mut next: Vec<&Self> = Vec::new();
        current.push(self);
        let mut level = 0;
        while !current.is_empty() {
            next.clear();
            write!(f, "level {level}:")?;
            for node in &current {
                write!(f, " [")?;
                for metric in &node.metrics {
                    write!(f, "({metric}) ")?;
                }
                write!(f, "]")?;
                if let Node::Internal(children) = &node.children {
                    for child in children {
                        next.push(child);
                    }
                }
            }
            writeln!(f)?;
            level += 1;
            std::mem::swap(&mut current, &mut next);
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct Leaf {
    parent: Option<NonNull<Internal>>,
}

impl Leaf {
    fn new(parent: NonNull<Internal>) -> Self {
        Self {
            parent: Some(parent),
        }
    }

    fn clear_parent(mut self: Box<Self>) -> Box<Self> {
        self.parent = None;
        self
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
struct Metric {
    bytes: usize,
    chars: usize,
}

impl Display for Metric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "b:{}, c:{}", self.bytes, self.chars)
    }
}

impl Sum for Metric {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |a, b| Self {
            bytes: a.bytes + b.bytes,
            chars: a.chars + b.chars,
        })
    }
}

impl Add for Metric {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            bytes: self.bytes + rhs.bytes,
            chars: self.chars + rhs.chars,
        }
    }
}

impl AddAssign for Metric {
    fn add_assign(&mut self, rhs: Self) {
        self.bytes += rhs.bytes;
        self.chars += rhs.chars;
    }
}

impl SubAssign for Metric {
    fn sub_assign(&mut self, rhs: Self) {
        self.bytes -= rhs.bytes;
        self.chars -= rhs.chars;
    }
}

impl Sub for Metric {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            bytes: self.bytes - rhs.bytes,
            chars: self.chars - rhs.chars,
        }
    }
}

fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod test {
    use super::*;

    fn metric(x: usize) -> Metric {
        Metric {
            bytes: x * 2,
            chars: x,
        }
    }

    #[test]
    fn test_insert() {
        let mut root = new_root();
        root.insert(metric(10));
        println!("{}", root);
        root.insert(metric(5));
        for i in 0..10 {
            println!("pushing {i}");
            root.insert(metric(i));
            println!("{}", root);
        }
    }

    #[test]
    fn test_push() {
        let mut root = new_root();
        println!("{}", root);
        for i in 1..20 {
            println!("pushing {i}");
            root.insert(metric(i));
            println!("{}", root);
        }
    }

    #[test]
    fn test_search() {
        let mut root = new_root();
        for i in 1..20 {
            root.insert(metric(i));
        }
        for i in 0..20 {
            println!("searching for {i}");
            let metric = root.search_byte(i * 2);
            assert_eq!(metric.chars, i);
        }
    }

    #[test]
    fn test_search_chars() {
        let mut root = new_root();
        for i in 1..20 {
            root.insert(metric(i));
        }
        for i in 0..20 {
            println!("searching for {i}");
            let metric = root.search_char(i);
            assert_eq!(metric.bytes, i * 2);
        }
    }
}

use std::sync::Arc;

pub trait Measured: Clone {
    fn units(&self) -> usize;
    fn render_blocks(&self) -> usize {
        1
    }
}

#[derive(Clone, Debug)]
pub struct Sequence<T>(Option<Arc<Tree<T>>>);

#[derive(Debug)]
enum Tree<T> {
    Leaf(T),
    Branch {
        left: Sequence<T>,
        right: Sequence<T>,
        count: usize,
        units: usize,
        render_blocks: usize,
        height: u32,
    },
}

impl<T> Default for Sequence<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T: Measured> Sequence<T> {
    pub fn one(value: T) -> Self {
        Self(Some(Arc::new(Tree::Leaf(value))))
    }

    pub fn from_items(items: impl IntoIterator<Item = T>) -> Self {
        items
            .into_iter()
            .fold(Self::default(), |tree, item| tree.concat(&Self::one(item)))
    }

    pub fn len(&self) -> usize {
        match self.0.as_deref() {
            None => 0,
            Some(Tree::Leaf(_)) => 1,
            Some(Tree::Branch { count, .. }) => *count,
        }
    }

    pub fn units(&self) -> usize {
        match self.0.as_deref() {
            None => 0,
            Some(Tree::Leaf(value)) => value.units(),
            Some(Tree::Branch { units, .. }) => *units,
        }
    }

    pub fn render_blocks(&self) -> usize {
        match self.0.as_deref() {
            None => 0,
            Some(Tree::Leaf(value)) => value.render_blocks(),
            Some(Tree::Branch { render_blocks, .. }) => *render_blocks,
        }
    }

    pub fn render_prefix(&self, index: usize) -> usize {
        match self.0.as_deref() {
            None => 0,
            Some(Tree::Leaf(value)) => {
                if index == 0 {
                    0
                } else {
                    value.render_blocks()
                }
            }
            Some(Tree::Branch { left, right, .. }) => {
                if index <= left.len() {
                    left.render_prefix(index)
                } else {
                    left.render_blocks() + right.render_prefix(index - left.len())
                }
            }
        }
    }

    pub fn locate_render(&self, offset: usize) -> Option<(usize, usize, &T)> {
        match self.0.as_deref()? {
            Tree::Leaf(value) => (offset < value.render_blocks()).then_some((0, offset, value)),
            Tree::Branch { left, right, .. } => {
                if offset < left.render_blocks() {
                    left.locate_render(offset)
                } else {
                    right
                        .locate_render(offset - left.render_blocks())
                        .map(|(index, at, value)| (index + left.len(), at, value))
                }
            }
        }
    }

    fn height(&self) -> u32 {
        match self.0.as_deref() {
            None => 0,
            Some(Tree::Leaf(_)) => 1,
            Some(Tree::Branch { height, .. }) => *height,
        }
    }

    fn branch(left: Self, right: Self) -> Self {
        if left.len() == 0 {
            return right;
        }
        if right.len() == 0 {
            return left;
        }
        Self(Some(Arc::new(Tree::Branch {
            count: left.len() + right.len(),
            units: left.units() + right.units(),
            render_blocks: left.render_blocks() + right.render_blocks(),
            height: 1 + left.height().max(right.height()),
            left,
            right,
        })))
    }

    fn balance(left: Self, right: Self) -> Self {
        if left.height() > right.height() + 1 {
            let Some(Tree::Branch {
                left: a, right: b, ..
            }) = left.0.as_deref()
            else {
                unreachable!()
            };
            if a.height() >= b.height() {
                return Self::branch(a.clone(), Self::branch(b.clone(), right));
            }
            let Some(Tree::Branch {
                left: c, right: d, ..
            }) = b.0.as_deref()
            else {
                unreachable!()
            };
            return Self::branch(
                Self::branch(a.clone(), c.clone()),
                Self::branch(d.clone(), right),
            );
        }
        if right.height() > left.height() + 1 {
            let Some(Tree::Branch {
                left: a, right: b, ..
            }) = right.0.as_deref()
            else {
                unreachable!()
            };
            if b.height() >= a.height() {
                return Self::branch(Self::branch(left, a.clone()), b.clone());
            }
            let Some(Tree::Branch {
                left: c, right: d, ..
            }) = a.0.as_deref()
            else {
                unreachable!()
            };
            return Self::branch(
                Self::branch(left, c.clone()),
                Self::branch(d.clone(), b.clone()),
            );
        }
        Self::branch(left, right)
    }

    pub fn concat(&self, other: &Self) -> Self {
        if self.height() > other.height() + 1 {
            let Some(Tree::Branch { left, right, .. }) = self.0.as_deref() else {
                unreachable!()
            };
            return Self::balance(left.clone(), right.concat(other));
        }
        if other.height() > self.height() + 1 {
            let Some(Tree::Branch { left, right, .. }) = other.0.as_deref() else {
                unreachable!()
            };
            return Self::balance(self.concat(left), right.clone());
        }
        Self::branch(self.clone(), other.clone())
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        match self.0.as_deref()? {
            Tree::Leaf(value) => (index == 0).then_some(value),
            Tree::Branch { left, right, .. } => {
                if index < left.len() {
                    left.get(index)
                } else {
                    right.get(index - left.len())
                }
            }
        }
    }

    pub fn prefix(&self, index: usize) -> usize {
        match self.0.as_deref() {
            None => 0,
            Some(Tree::Leaf(value)) => {
                if index == 0 {
                    0
                } else {
                    value.units()
                }
            }
            Some(Tree::Branch { left, right, .. }) => {
                if index <= left.len() {
                    left.prefix(index)
                } else {
                    left.units() + right.prefix(index - left.len())
                }
            }
        }
    }

    pub fn locate(&self, offset: usize) -> Option<(usize, usize, &T)> {
        match self.0.as_deref()? {
            Tree::Leaf(value) => Some((0, offset.min(value.units()), value)),
            Tree::Branch { left, right, .. } => {
                if offset < left.units() {
                    left.locate(offset)
                } else {
                    right
                        .locate(offset - left.units())
                        .map(|(i, at, value)| (i + left.len(), at, value))
                }
            }
        }
    }

    pub fn split(&self, count: usize) -> (Self, Self) {
        if count == 0 {
            return (Self::default(), self.clone());
        }
        if count >= self.len() {
            return (self.clone(), Self::default());
        }
        let Some(Tree::Branch { left, right, .. }) = self.0.as_deref() else {
            unreachable!()
        };
        if count < left.len() {
            let (a, b) = left.split(count);
            (a, b.concat(right))
        } else {
            let (a, b) = right.split(count - left.len());
            (left.concat(&a), b)
        }
    }

    pub fn splice(&self, range: std::ops::Range<usize>, replacement: &Self) -> Self {
        let (left, rest) = self.split(range.start);
        let (_, right) = rest.split(range.end - range.start);
        left.concat(replacement).concat(&right)
    }

    pub fn visit(&self, f: &mut impl FnMut(&T)) {
        match self.0.as_deref() {
            None => {}
            Some(Tree::Leaf(value)) => f(value),
            Some(Tree::Branch { left, right, .. }) => {
                left.visit(f);
                right.visit(f);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Measured for usize {
        fn units(&self) -> usize {
            *self
        }
    }

    #[test]
    fn persistent_balanced_splices_and_measurements() {
        let original = Sequence::from_items(1..=10_000);
        let mut tree = original.clone();
        let mut oracle: Vec<usize> = (1..=10_000).collect();
        for i in 0..2_000 {
            let at = (i * 7919) % oracle.len();
            tree = tree.splice(at..at + 1, &Sequence::from_items([2, 3]));
            oracle.splice(at..at + 1, [2, 3]);
            assert_eq!(tree.units(), oracle.iter().sum::<usize>());
            assert!(tree.height() < 24);
        }
        assert_eq!(original.len(), 10_000);
        for (i, value) in oracle.iter().enumerate() {
            assert_eq!(tree.get(i), Some(value));
            let start = tree.prefix(i);
            assert_eq!(tree.locate(start).map(|x| x.0), Some(i));
        }
    }
}

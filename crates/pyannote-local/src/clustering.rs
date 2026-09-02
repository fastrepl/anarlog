//! Agglomerative clustering of speaker embeddings, following the
//! centroid-linkage recipe used by pyannote.audio 3.1: embeddings are
//! unit-normalised so euclidean distance is a monotone function of cosine
//! distance, the dendrogram is cut at a fixed threshold, and clusters smaller
//! than `min_cluster_size` are absorbed by the nearest large cluster.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpeakerBounds {
    pub num_speakers: Option<usize>,
    pub min_speakers: Option<usize>,
    pub max_speakers: Option<usize>,
}

impl SpeakerBounds {
    pub fn exact(num_speakers: usize) -> Self {
        Self {
            num_speakers: Some(num_speakers),
            ..Default::default()
        }
    }

    fn range(&self) -> (usize, usize) {
        match self.num_speakers {
            Some(count) => (count.max(1), count.max(1)),
            None => {
                let min = self.min_speakers.unwrap_or(1).max(1);
                let max = self.max_speakers.unwrap_or(usize::MAX).max(min);
                (min, max)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Merge {
    pub left: usize,
    pub right: usize,
    pub distance: f32,
}

pub fn normalize(vector: &[f32]) -> Vec<f32> {
    let norm = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm == 0.0 || !norm.is_finite() {
        return vector.to_vec();
    }
    vector.iter().map(|v| v / norm).collect()
}

fn euclidean(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(l, r)| (l - r) * (l - r))
        .sum::<f32>()
        .sqrt()
}

pub fn cosine_distance(left: &[f32], right: &[f32]) -> f32 {
    let dot = left.iter().zip(right).map(|(l, r)| l * r).sum::<f32>();
    let norm = left.iter().map(|v| v * v).sum::<f32>().sqrt()
        * right.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm == 0.0 {
        return 1.0;
    }
    (1.0 - dot / norm).clamp(0.0, 2.0)
}

/// Centroid-linkage dendrogram over unit-normalised embeddings. Returns the
/// `n - 1` merges in the order they happened.
pub fn linkage(embeddings: &[Vec<f32>]) -> Vec<Merge> {
    let n = embeddings.len();
    if n < 2 {
        return Vec::new();
    }
    let unit: Vec<Vec<f32>> = embeddings.iter().map(|e| normalize(e)).collect();
    let dim = unit[0].len();

    let mut sums = unit.clone();
    let mut sizes = vec![1usize; n];
    let mut alive = vec![true; n];
    let mut dist = vec![f32::INFINITY; n * n];
    for i in 0..n {
        for j in (i + 1)..n {
            let d = euclidean(&unit[i], &unit[j]);
            dist[i * n + j] = d;
            dist[j * n + i] = d;
        }
    }
    let nearest = |dist: &[f32], alive: &[bool], row: usize| -> (usize, f32) {
        let mut best = (usize::MAX, f32::INFINITY);
        for col in 0..n {
            if col != row && alive[col] && dist[row * n + col] < best.1 {
                best = (col, dist[row * n + col]);
            }
        }
        best
    };
    let mut nn: Vec<(usize, f32)> = (0..n).map(|row| nearest(&dist, &alive, row)).collect();

    let mut merges = Vec::with_capacity(n - 1);
    let mut centroid = vec![0.0f32; dim];
    let mut other = vec![0.0f32; dim];
    for _ in 0..(n - 1) {
        let mut best = (usize::MAX, usize::MAX, f32::INFINITY);
        for row in 0..n {
            if alive[row] && nn[row].1 < best.2 {
                best = (row, nn[row].0, nn[row].1);
            }
        }
        let (left, right, distance) = best;
        if left == usize::MAX || right == usize::MAX {
            break;
        }
        let (keep, drop) = (left.min(right), left.max(right));
        merges.push(Merge {
            left: keep,
            right: drop,
            distance,
        });

        let dropped = std::mem::take(&mut sums[drop]);
        for (sum, value) in sums[keep].iter_mut().zip(&dropped) {
            *sum += value;
        }
        sizes[keep] += sizes[drop];
        alive[drop] = false;

        for d in 0..dim {
            centroid[d] = sums[keep][d] / sizes[keep] as f32;
        }
        for col in 0..n {
            if col == keep || !alive[col] {
                continue;
            }
            for d in 0..dim {
                other[d] = sums[col][d] / sizes[col] as f32;
            }
            let d = euclidean(&centroid, &other);
            dist[keep * n + col] = d;
            dist[col * n + keep] = d;
        }
        for col in 0..n {
            if !alive[col] {
                continue;
            }
            if col == keep || nn[col].0 == keep || nn[col].0 == drop {
                nn[col] = nearest(&dist, &alive, col);
            } else if dist[col * n + keep] < nn[col].1 {
                nn[col] = (keep, dist[col * n + keep]);
            }
        }
    }
    merges
}

/// Labels after applying the first `merge_count` merges, compacted to
/// `0..k` in order of first appearance.
pub fn cut(item_count: usize, merges: &[Merge], merge_count: usize) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..item_count).collect();
    fn find(parent: &mut [usize], mut index: usize) -> usize {
        while parent[index] != index {
            parent[index] = parent[parent[index]];
            index = parent[index];
        }
        index
    }
    for merge in merges.iter().take(merge_count) {
        let left = find(&mut parent, merge.left);
        let right = find(&mut parent, merge.right);
        if left != right {
            parent[right] = left;
        }
    }
    let mut labels = Vec::with_capacity(item_count);
    let mut lookup: Vec<Option<usize>> = vec![None; item_count];
    let mut next = 0;
    for index in 0..item_count {
        let root = find(&mut parent, index);
        let label = *lookup[root].get_or_insert_with(|| {
            let label = next;
            next += 1;
            label
        });
        labels.push(label);
    }
    labels
}

pub fn centroids(embeddings: &[Vec<f32>], labels: &[usize]) -> Vec<Vec<f32>> {
    let count = labels.iter().copied().max().map_or(0, |max| max + 1);
    let dim = embeddings.first().map_or(0, |e| e.len());
    let mut sums = vec![vec![0.0f32; dim]; count];
    let mut sizes = vec![0usize; count];
    for (embedding, &label) in embeddings.iter().zip(labels) {
        let unit = normalize(embedding);
        for (sum, value) in sums[label].iter_mut().zip(&unit) {
            *sum += value;
        }
        sizes[label] += 1;
    }
    sums.into_iter()
        .zip(sizes)
        .map(|(sum, size)| {
            if size == 0 {
                sum
            } else {
                sum.into_iter().map(|v| v / size as f32).collect()
            }
        })
        .collect()
}

pub fn nearest_centroid(embedding: &[f32], centroids: &[Vec<f32>]) -> Option<usize> {
    let unit = normalize(embedding);
    centroids
        .iter()
        .enumerate()
        .map(|(index, centroid)| (index, euclidean(&unit, centroid)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(index, _)| index)
}

fn cluster_sizes(labels: &[usize]) -> Vec<usize> {
    let count = labels.iter().copied().max().map_or(0, |max| max + 1);
    let mut sizes = vec![0usize; count];
    for &label in labels {
        sizes[label] += 1;
    }
    sizes
}

fn large_cluster_count(labels: &[usize], min_cluster_size: usize) -> usize {
    cluster_sizes(labels)
        .into_iter()
        .filter(|size| *size >= min_cluster_size)
        .count()
}

/// Clusters `embeddings` and returns one label per embedding. Small clusters
/// are absorbed by the nearest large one so the label count reflects the
/// number of speakers actually supported by evidence.
pub fn cluster(
    embeddings: &[Vec<f32>],
    threshold: f32,
    min_cluster_size: usize,
    bounds: SpeakerBounds,
) -> Vec<usize> {
    let n = embeddings.len();
    if n == 0 {
        return Vec::new();
    }
    let min_cluster_size = min_cluster_size.max(1);
    let merges = linkage(embeddings);
    let (min_clusters, max_clusters) = bounds.range();

    let threshold_cut = merges
        .iter()
        .position(|merge| merge.distance > threshold)
        .unwrap_or(merges.len());
    let mut chosen = threshold_cut;
    let mut count = large_cluster_count(&cut(n, &merges, chosen), min_cluster_size);
    if count < min_clusters || count > max_clusters {
        let target = count.clamp(min_clusters, max_clusters);
        let mut best: Option<(usize, usize)> = None;
        for offset in 0..=merges.len() {
            for candidate in [
                threshold_cut.checked_sub(offset),
                Some(threshold_cut + offset),
            ]
            .into_iter()
            .flatten()
            .filter(|candidate| *candidate <= merges.len())
            {
                let candidate_count =
                    large_cluster_count(&cut(n, &merges, candidate), min_cluster_size);
                let gap = candidate_count.abs_diff(target);
                if best.is_none_or(|(_, best_gap)| gap < best_gap) {
                    best = Some((candidate, gap));
                }
                if gap == 0 {
                    break;
                }
            }
            if best.is_some_and(|(_, gap)| gap == 0) {
                break;
            }
        }
        if let Some((candidate, _)) = best {
            chosen = candidate;
            count = large_cluster_count(&cut(n, &merges, chosen), min_cluster_size);
        }
    }

    let labels = cut(n, &merges, chosen);
    if count == 0 {
        return vec![0; n];
    }

    let sizes = cluster_sizes(&labels);
    let large: Vec<usize> = (0..sizes.len())
        .filter(|label| sizes[*label] >= min_cluster_size)
        .collect();
    let large_centroids: Vec<Vec<f32>> = {
        let all = centroids(embeddings, &labels);
        large.iter().map(|label| all[*label].clone()).collect()
    };
    labels
        .iter()
        .zip(embeddings)
        .map(
            |(&label, embedding)| match large.iter().position(|l| *l == label) {
                Some(index) => index,
                None => nearest_centroid(embedding, &large_centroids).unwrap_or(0),
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f32, y: f32) -> Vec<f32> {
        vec![x, y]
    }

    fn two_groups() -> Vec<Vec<f32>> {
        vec![
            point(1.0, 0.0),
            point(0.98, 0.05),
            point(0.99, -0.03),
            point(0.0, 1.0),
            point(0.05, 0.98),
            point(-0.03, 0.99),
        ]
    }

    #[test]
    fn linkage_produces_n_minus_one_merges() {
        let merges = linkage(&two_groups());
        assert_eq!(merges.len(), 5);
        assert!(merges.last().unwrap().distance > merges[0].distance);
    }

    #[test]
    fn threshold_separates_distinct_groups() {
        let labels = cluster(&two_groups(), 0.7, 1, SpeakerBounds::default());
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[1], labels[2]);
        assert_eq!(labels[3], labels[4]);
        assert_eq!(labels[4], labels[5]);
        assert_ne!(labels[0], labels[3]);
    }

    #[test]
    fn exact_speaker_count_overrides_threshold() {
        let one = cluster(&two_groups(), 0.7, 1, SpeakerBounds::exact(1));
        assert!(one.iter().all(|label| *label == 0));

        let three = cluster(&two_groups(), 0.01, 1, SpeakerBounds::exact(3));
        let distinct = three.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(distinct.len(), 3);
    }

    #[test]
    fn max_speakers_caps_cluster_count() {
        let labels = cluster(
            &two_groups(),
            0.01,
            1,
            SpeakerBounds {
                max_speakers: Some(2),
                ..Default::default()
            },
        );
        let distinct = labels.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(distinct.len(), 2);
    }

    #[test]
    fn small_clusters_are_absorbed() {
        let mut embeddings = two_groups();
        embeddings.push(point(-1.0, 0.0));
        let labels = cluster(&embeddings, 0.7, 2, SpeakerBounds::default());
        let distinct = labels.iter().collect::<std::collections::HashSet<_>>();
        assert_eq!(distinct.len(), 2);
        assert_eq!(labels[6], labels[3]);
    }

    #[test]
    fn single_embedding_is_one_cluster() {
        assert_eq!(
            cluster(&[point(1.0, 0.0)], 0.7, 12, SpeakerBounds::default()),
            vec![0]
        );
        assert!(cluster(&[], 0.7, 12, SpeakerBounds::default()).is_empty());
    }

    #[test]
    fn cut_labels_are_compact() {
        let embeddings = two_groups();
        let merges = linkage(&embeddings);
        let labels = cut(embeddings.len(), &merges, 4);
        let max = *labels.iter().max().unwrap();
        assert_eq!(max, 1);
    }
}

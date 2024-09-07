use std::collections::BTreeSet;

// TODO: remove segments when empty?
// TODO: shrink archive when needed?
// TODO: update "len" when required
// TODO: shrink archives when possible

/// Compute which parts of an archive's memory is used or not
///
/// Allows to quickly find unused space, compute wasted space, and shrink the archive if necessary
pub(crate) struct Coverage {
    len: u64,
    segments: BTreeSet<Segment>,
}

impl Coverage {
    pub fn new(len: u64) -> Self {
        Self {
            len,
            segments: BTreeSet::new(),
        }
    }

    pub fn grow_to(&mut self, new_len: u64) {
        assert!(new_len >= self.len);
        self.len = new_len;
    }

    // TODO: shrink(&mut self, by: u64)

    /// Mark a zone as used
    pub fn mark_as_used(&mut self, start: u64, len: u64) {
        if len == 0 {
            return;
        }

        if let Some(prev) = self.segments.iter().find(|segment| segment.start <= start) {
            assert!(prev.start + prev.len <= start);
        }

        if let Some(next) = self.segments.iter().find(|segment| segment.start >= start) {
            assert!(next.start + next.len >= start + len);
        }

        self.segments.insert(Segment { start, len });
    }

    /// Mark as zone as free (unused)
    pub fn mark_as_free(&mut self, segment: Segment) {
        if segment.len > 0 {
            // TODO: support non-exact segments
            assert!(self.segments.remove(&segment));
        }
    }

    /// Find the next free (unused) zones
    pub fn find_free_zones(&self) -> FreeSegmentsIter {
        FreeSegmentsIter::new(self)
    }

    /// Find the smallest segment with at least the provided capacity
    pub fn find_free_zone_for(&self, capacity: u64) -> Option<Segment> {
        self.find_free_zones()
            .filter(|zone| zone.len >= capacity)
            .min_by_key(|zone| zone.len)
    }

    /// Find the next writable address (after every segment)
    pub fn next_writable_addr(&self) -> u64 {
        match self.segments.last() {
            Some(last) => last.start + last.len,
            None => 0,
        }
    }
}

/// Representation of a segment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub start: u64,
    pub len: u64,
}

impl Ord for Segment {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.start.cmp(&other.start)
    }
}

impl PartialOrd for Segment {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Iterator over a list of free segments
pub struct FreeSegmentsIter<'a> {
    covering: &'a Coverage,
    step: usize,
}

impl<'a> FreeSegmentsIter<'a> {
    fn new(covering: &'a Coverage) -> Self {
        Self { covering, step: 0 }
    }
}

impl<'a> Iterator for FreeSegmentsIter<'a> {
    type Item = Segment;

    fn next(&mut self) -> Option<Self::Item> {
        if self.step == 0 {
            self.step += 1;

            match self.covering.segments.first() {
                Some(first) => {
                    if first.start > 0 {
                        Some(Segment {
                            start: 0,
                            len: first.start,
                        })
                    } else {
                        self.next()
                    }
                }

                None => {
                    if self.covering.len > 0 {
                        Some(Segment {
                            start: 0,
                            len: self.covering.len,
                        })
                    } else {
                        None
                    }
                }
            }
        } else if self.step == self.covering.segments.len() {
            self.step += 1;

            let last = self.covering.segments.last().unwrap();
            let free_from = last.start + last.len;

            if free_from < self.covering.len {
                Some(Segment {
                    start: free_from,
                    len: self.covering.len - free_from,
                })
            } else {
                None
            }
        } else if self.step == self.covering.segments.len() + 1 {
            None
        } else {
            self.step += 1;

            let prev = self.covering.segments.iter().nth(self.step - 2).unwrap();
            let curr = self.covering.segments.iter().nth(self.step - 1).unwrap();

            let prev_end = prev.start + prev.len;

            if prev_end < curr.start {
                Some(Segment {
                    start: prev_end,
                    len: curr.start - prev_end,
                })
            } else {
                self.next()
            }
        }
    }
}

//! Which parts of a page's pictures changed from paint to paint, so the window copies and uploads only those.

use std::collections::VecDeque;

use crate::frame::Region;

/// How many paints are remembered; a picture older than that is copied whole.
const REMEMBERED: usize = 32;

/// The parts of the picture each recent paint of a page changed.
#[derive(Debug, Default)]
pub(crate) struct Damage {
    /// The size of a picture in the files painted into now.
    size: (u32, u32),
    /// Each remembered paint's generation and the part it changed, oldest first, without gaps.
    paints: VecDeque<(u64, Region)>,
    /// Whether no paint went into the files painted into now yet, so the next one changes the whole picture.
    fresh: bool,
}

impl Damage {
    /// Forgets every paint, since the page paints into new files `width` by `height` pixels from now on.
    pub(crate) fn surface(&mut self, width: u32, height: u32) {
        self.size = (width, height);
        self.paints.clear();
        self.fresh = true;
    }

    /// Remembers that the paint `generation` changed `dirty`, the left, top, width and height the helper reported, or
    /// the whole picture when it reported none or the paint is the first into new files.
    pub(crate) fn painted(&mut self, generation: u64, dirty: Option<[u32; 4]>) {
        let (width, height) = self.size;
        let region = dirty
            .filter(|_| !self.fresh)
            .map_or(Region::whole(width, height), |dirty| {
                Region::reported(dirty, width, height)
            });
        self.fresh = false;
        if self
            .paints
            .back()
            .is_some_and(|(last, _)| generation != last + 1)
        {
            self.paints.clear();
        }
        self.paints.push_back((generation, region));
        if self.paints.len() > REMEMBERED {
            self.paints.pop_front();
        }
    }

    /// The part of the picture that may differ between the paints `from` and `to`, or nothing when a paint between
    /// them is not remembered, so the whole picture is to be taken.
    pub(crate) fn since(&self, from: u64, to: u64) -> Option<Region> {
        if from > to {
            return None;
        }
        if from == to {
            return Some(Region::default());
        }
        let first = self.paints.front()?.0;
        if from + 1 < first {
            return None;
        }
        let mut covered = from;
        let mut region = Region::default();
        for (generation, changed) in &self.paints {
            if *generation > from && *generation <= to {
                region = region.union(*changed);
                covered = *generation;
            }
        }
        (covered == to).then_some(region)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The parts of consecutive paints add up, a gap or a new surface takes the whole picture, and the first paint
    /// into new files changes all of it.
    #[test]
    fn paints_add_up_until_one_is_missing() {
        let mut damage = Damage::default();
        damage.surface(100, 50);
        damage.painted(1, Some([10, 10, 5, 5]));
        assert_eq!(damage.since(0, 1), Some(Region::whole(100, 50)));
        damage.painted(2, Some([10, 10, 5, 5]));
        damage.painted(3, Some([20, 0, 5, 200]));
        assert_eq!(
            damage.since(1, 3),
            Some(Region {
                x: 10,
                y: 0,
                width: 15,
                height: 50
            })
        );
        assert_eq!(damage.since(3, 3), Some(Region::default()));
        assert_eq!(damage.since(3, 4), None);
        assert_eq!(damage.since(4, 3), None);
        damage.painted(9, None);
        assert_eq!(damage.since(3, 9), None);
        damage.painted(10, Some([0, 0, 1, 1]));
        assert_eq!(damage.since(9, 10), Some(Region::whole(1, 1)));
        damage.surface(10, 10);
        damage.painted(11, Some([0, 0, 1, 1]));
        assert_eq!(damage.since(10, 11), Some(Region::whole(10, 10)));
    }
}

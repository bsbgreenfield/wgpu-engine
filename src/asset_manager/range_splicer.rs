use std::ops::Range;

#[derive(PartialEq, PartialOrd, Debug)]
enum IndexResult {
    StrictlyGreater,
    StrictlyLess,
    FullyContained,
    FullyCover,
    LeftOverlap,
    RightOverlap,
    ExactlyEqual,
}
fn get_index_result(
    current_range: &Range<usize>,
    primitive_indices_range: &Range<usize>,
) -> IndexResult {
    if primitive_indices_range.start == current_range.start
        && primitive_indices_range.end == primitive_indices_range.end
    {
        IndexResult::ExactlyEqual
    } else if primitive_indices_range.start > current_range.end + 1 {
        IndexResult::StrictlyGreater
    } else if primitive_indices_range.end + 1 < current_range.start {
        IndexResult::StrictlyLess
    } else if primitive_indices_range.start == current_range.end + 1 {
        // we consider this case a right overlap, because the result is the same
        IndexResult::RightOverlap
    } else if primitive_indices_range.end + 1 == current_range.start {
        // consider this a left overlap
        IndexResult::LeftOverlap
    } else if primitive_indices_range.start > current_range.start
        && primitive_indices_range.end > current_range.end
    {
        IndexResult::RightOverlap
    } else if primitive_indices_range.start < current_range.start
        && primitive_indices_range.end < current_range.end
    {
        IndexResult::LeftOverlap
    } else if primitive_indices_range.start > current_range.start
        && primitive_indices_range.end < current_range.end
    {
        IndexResult::FullyContained
    } else if primitive_indices_range.start < current_range.start
        && primitive_indices_range.end > current_range.end
    {
        IndexResult::FullyCover
    } else {
        panic!("{:?}, {:?}", primitive_indices_range, current_range)
    }
}

fn splice_range(
    range_vec: &mut Vec<Range<usize>>,
    primitive_indices_range: &Range<usize>,
    start_idx: usize,
    end_idx: usize,
) {
    let left = std::cmp::min(primitive_indices_range.start, range_vec[start_idx].start);
    let right = std::cmp::max(primitive_indices_range.end, range_vec[end_idx - 1].end);
    range_vec.splice(start_idx..end_idx, vec![left..right]);
}

pub(super) fn define_index_ranges(
    range_vec: &mut Vec<Range<usize>>,
    primitive_indices_range: &std::ops::Range<usize>,
) {
    if range_vec.is_empty() {
        range_vec.push(primitive_indices_range.clone());
        return;
    }
    let mut current_idx: usize = 0;
    let mut range_iter = range_vec.clone().into_iter().peekable();
    'outer: loop {
        if let Some(current_range) = range_iter.next() {
            let index_result = get_index_result(&current_range, &primitive_indices_range);
            use IndexResult::*;
            match index_result {
                ExactlyEqual => {
                    break 'outer;
                }
                // if this range is strictly greater than the current range, and this is the last
                // range, add a new range to the end, unless we can combine it into a continuous
                // new range
                StrictlyGreater => {
                    if range_iter.peek().is_none() {
                        range_vec.push(std::ops::Range {
                            start: primitive_indices_range.start,
                            end: primitive_indices_range.end,
                        });
                        break 'outer;
                    }
                    current_idx += 1;
                    continue 'outer;
                }
                // if this range is strictly less than the curent range it needs to be placed
                // before the current range
                StrictlyLess => {
                    range_vec.insert(current_idx, primitive_indices_range.clone());
                    break 'outer;
                }

                // if the primitive range overlaps the left side of this current range, replace the
                // current range with a new one that spans backwards to include the start of this
                // primitives range
                LeftOverlap => {
                    splice_range(
                        range_vec,
                        &primitive_indices_range,
                        current_idx,
                        current_idx + 1,
                    );
                    break 'outer;
                }
                // do nothing, we are already covered
                FullyContained => {}
                // if this primirives range fully encapsulates the current range, or it overlaps
                // it on the right side, the end index may still overlap with some of the other
                // ranges. loop through until we either
                // 1. hit the end of the range vec -> replace all subsequent ranges with new
                // 2. find a left overlap, splice in a vec which spans from the start of the vec in
                //    which we first found to the end of the new one
                FullyCover | RightOverlap => {
                    let mut splice_offset = 0;
                    'inner: loop {
                        splice_offset += 1;
                        if range_iter.peek().is_none() {
                            splice_range(
                                range_vec,
                                &primitive_indices_range,
                                current_idx,
                                current_idx + splice_offset,
                            );
                            break 'outer;
                        }
                        let next_range = range_iter.next().unwrap();
                        let next_index_result =
                            get_index_result(&next_range, &primitive_indices_range);
                        if next_index_result == FullyCover {
                            continue 'inner;
                        }
                        if next_index_result == LeftOverlap {
                            splice_range(
                                range_vec,
                                &primitive_indices_range,
                                current_idx,
                                current_idx + splice_offset + 1,
                            );
                            break 'outer;
                        } else if next_index_result == StrictlyLess {
                            splice_range(
                                range_vec,
                                &primitive_indices_range,
                                current_idx,
                                current_idx + splice_offset,
                            );
                            break 'outer;
                        } else {
                            panic!();
                        }
                    }
                }
            }
        } else {
            return;
        }
        current_idx += 1;
    }
}
#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_new_strictly_greater() {
        let mut indices_ranges = Vec::<Range<usize>>::new();
        indices_ranges.push(0..4);
        indices_ranges.push(7..10);
        let primitive_indices_range = Range { start: 12, end: 15 };
        define_index_ranges(&mut indices_ranges, &primitive_indices_range);

        assert_eq!(indices_ranges.len(), 3);
        assert_eq!(
            *indices_ranges.last().unwrap(),
            Range { start: 12, end: 15 }
        );
    }

    #[test]
    fn test_new_right_overlap_1() {
        let mut indices_ranges = Vec::<Range<usize>>::new();
        indices_ranges.push(0..4);
        indices_ranges.push(7..10);
        let primitive_indices_range = Range { start: 8, end: 15 };
        define_index_ranges(&mut indices_ranges, &primitive_indices_range);
        assert_eq!(indices_ranges.len(), 2);
        assert_eq!(indices_ranges[0], Range { start: 0, end: 4 });
        assert_eq!(indices_ranges[1], Range { start: 7, end: 15 });
    }

    #[test]
    fn test_new_right_overlap_2() {
        let mut indices_ranges = Vec::<Range<usize>>::new();
        indices_ranges.push(0..4);
        indices_ranges.push(7..10);
        let primitive_indices_range = Range { start: 10, end: 15 };
        define_index_ranges(&mut indices_ranges, &primitive_indices_range);
        assert_eq!(indices_ranges.len(), 2);
        assert_eq!(indices_ranges[0], Range { start: 0, end: 4 });
        assert_eq!(indices_ranges[1], Range { start: 7, end: 15 });
    }
    #[test]
    fn test_right_overlap_3() {
        let mut indices_ranges = Vec::<Range<usize>>::new();
        indices_ranges.push(2..4);
        indices_ranges.push(8..10);
        indices_ranges.push(12..15);
        let primitive_indices_range = Range { start: 3, end: 13 };
        define_index_ranges(&mut indices_ranges, &primitive_indices_range);
        assert_eq!(indices_ranges.len(), 1);
        assert_eq!(indices_ranges[0], Range { start: 2, end: 15 });
    }

    #[test]
    fn test_full_emcompass() {
        let mut indices_ranges = Vec::<Range<usize>>::new();
        indices_ranges.push(2..4);
        indices_ranges.push(7..10);
        let primitive_indices_range = Range { start: 0, end: 15 };
        define_index_ranges(&mut indices_ranges, &primitive_indices_range);
        assert_eq!(indices_ranges.len(), 1);
        assert_eq!(indices_ranges[0], Range { start: 0, end: 15 });
    }
    #[test]
    fn test_part_emcompass() {
        let mut indices_ranges = Vec::<Range<usize>>::new();
        indices_ranges.push(2..4);
        indices_ranges.push(10..18);
        indices_ranges.push(22..25);
        let primitive_indices_range = Range { start: 6, end: 20 };
        define_index_ranges(&mut indices_ranges, &primitive_indices_range);
        assert_eq!(indices_ranges.len(), 3);
        assert_eq!(indices_ranges[0], Range { start: 2, end: 4 });
        assert_eq!(indices_ranges[1], Range { start: 6, end: 20 });
        assert_eq!(indices_ranges[2], Range { start: 22, end: 25 });
    }

    #[test]
    fn test_new_left_overlap_1() {
        let mut indices_ranges = Vec::<Range<usize>>::new();
        indices_ranges.push(2..4);
        indices_ranges.push(8..10);
        indices_ranges.push(12..15);
        let primitive_indices_range = Range { start: 6, end: 9 };
        define_index_ranges(&mut indices_ranges, &primitive_indices_range);
        assert_eq!(indices_ranges.len(), 3);
        assert_eq!(indices_ranges[0], Range { start: 2, end: 4 });
        assert_eq!(indices_ranges[1], Range { start: 6, end: 10 });
        assert_eq!(indices_ranges[2], Range { start: 12, end: 15 });
    }
    #[test]
    fn test_new_in_betweener() {
        let mut indices_ranges = Vec::<Range<usize>>::new();
        indices_ranges.push(2..4);
        indices_ranges.push(12..20);
        let primitive_indices_range = Range { start: 6, end: 10 };
        define_index_ranges(&mut indices_ranges, &primitive_indices_range);
        assert_eq!(indices_ranges.len(), 3);
        assert_eq!(indices_ranges[0], Range { start: 2, end: 4 });
        assert_eq!(indices_ranges[1], Range { start: 6, end: 10 });
        assert_eq!(indices_ranges[2], Range { start: 12, end: 20 });
    }
}

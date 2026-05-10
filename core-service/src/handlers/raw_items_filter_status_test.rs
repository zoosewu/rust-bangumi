use super::collect_filter_passed;

#[test]
fn collect_filter_passed_marks_raw_item_filtered_when_any_link_is_filtered() {
    let rows = vec![(Some(1), false), (Some(1), true), (Some(2), false)];

    let result = collect_filter_passed(rows);

    assert_eq!(result.get(&1), Some(&Some(false)));
    assert_eq!(result.get(&2), Some(&Some(true)));
}

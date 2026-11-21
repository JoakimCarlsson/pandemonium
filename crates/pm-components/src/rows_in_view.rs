//! Keeping the row the keys act on in view in a lazy list, the way a Compose list asks its `LazyListState` to scroll to an item only when it is out of view.

/// Scrolls `list` the least that brings item `index` wholly into view, as of its latest
/// layout: to the item's top when it is above the view, to its bottom when below, and not at
/// all when it is in view already.
pub(crate) fn keep_in_view(list: &rustle::LazyListState, index: usize) {
    let info = list.layout_info();
    let seen = info
        .visible_items_info
        .iter()
        .find(|item| item.index == index);
    match seen {
        Some(item)
            if item.offset >= info.viewport_start_offset
                && item.offset + item.size <= info.viewport_end_offset => {}
        Some(item) if item.offset < info.viewport_start_offset => {
            list.scroll_to_item(index, rustle::ItemAlign::Start)
        }
        Some(_) => list.scroll_to_item(index, rustle::ItemAlign::End),
        None if info
            .visible_items_info
            .first()
            .is_some_and(|first| index < first.index) =>
        {
            list.scroll_to_item(index, rustle::ItemAlign::Start)
        }
        None => list.scroll_to_item(index, rustle::ItemAlign::End),
    }
}

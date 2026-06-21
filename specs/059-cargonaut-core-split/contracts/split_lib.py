#!/usr/bin/env python3
"""One-shot, brace/string/comment-aware splitter for cargonaut-core/src/lib.rs.

Feature 059: move-only god-file split. Reads lib.rs, partitions every
top-level construct (types, impl blocks, free fns/consts) and every test fn
into responsibility submodules, and rewrites lib.rs as a thin module root
(crate docs + pub(crate) external re-exports + `App`/`SideState` + `mod`
decls + `pub use` surface). Verified afterward by the compiler + the 192-test
suite + the rustdoc-JSON public-surface diff.
"""
import re
import sys

SRC = "crates/cargonaut-core/src/lib.rs"
OUT_DIR = "crates/cargonaut-core/src"

# --- module routing -------------------------------------------------------

# Value types & their non-App impls route by the type name.
TYPE_MODULE = {
    "PaneId": "pane", "PaneFilter": "pane", "PaneState": "pane",
    "FocusedRow": "pane", "TabBarEntry": "pane", "ViewMode": "pane",
    "SplitOrient": "pane",
    "Command": "command", "Event": "command", "DialogKind": "command",
    "AppError": "error", "UndoEntry": "error",
    "JobStatus": "jobs", "JobView": "jobs", "ProgressView": "jobs",
    "ResumeOfferView": "jobs",
    # App and SideState stay in lib.rs (handled specially)
}

# impl App methods route by method name.
METHOD_MODULE = {
    # app core
    "new": "app", "registry": "app", "view_mode": "app",
    "active_progress": "app", "split_orient": "app", "config": "app",
    "active_pane": "app", "pane": "app", "active_pane_state": "app",
    "status": "app", "active_pane_mut": "app", "pane_mut": "app",
    "dispatch": "app",
    # nav
    "relist_active": "nav", "navigate_into": "nav", "refresh_active_pane": "nav",
    "descend_into_focused": "nav", "sync_other_panel_path": "nav",
    "show_focused_in_other_panel": "nav", "ascend_to_parent": "nav",
    "navigate_to": "nav", "resolve_cd_target": "nav", "quick_cd": "nav",
    "complete_cd": "nav", "selection_or_focused": "nav", "set_filter": "nav",
    # history
    "history_prev_dir": "history", "history_next_dir": "history",
    # fsops
    "mkdir": "fsops", "select_by_pattern": "fsops", "recursive_dir_size": "fsops",
    # attrs
    "chmod_selection": "attrs", "chown_selection": "attrs",
    "collect_subtree": "attrs", "collect_subtree_capped": "attrs",
    "chmod_recursive": "attrs", "chown_recursive": "attrs", "attr_roots": "attrs",
    "create_symlink": "attrs", "create_hard_link": "attrs", "link_source": "attrs",
    # compare
    "compare_directories": "compare",
    # rename
    "undo_last_operation": "rename", "apply_bulk_rename": "rename",
    # hotlist
    "bookmarks": "hotlist", "add_bookmark": "hotlist", "remove_bookmark": "hotlist",
    "jump_to_bookmark": "hotlist", "persist_hotlist": "hotlist",
    # tabs
    "tab_new": "tabs", "tab_close": "tabs", "tab_next": "tabs",
    "tab_prev": "tabs", "tab_bar_view": "tabs",
    # transfers
    "transfer_ids": "transfers", "transfer": "transfers", "job_views": "transfers",
    "cancel_transfer": "transfers", "pause_transfer": "transfers",
    "resume_paused": "transfers", "confirm_copy": "transfers",
    "transfer_opts": "transfers", "scan_resume_offers": "transfers",
    "pending_resume_views": "transfers", "resume_offer": "transfers",
    "start_over_offer": "transfers", "skip_offer": "transfers",
    "request_copy_confirmation": "transfers",
    "request_move_confirmation": "transfers",
    "request_delete_confirmation": "transfers",
}

# Free fns / consts route by name. Value = (module, keep_pub_visibility?)
FREEFN_MODULE = {
    "validate_rename_proposals": ("rename", "pub"),
    "glob_match": ("pane", "pub"),
    "transfer_state_snapshot": ("jobs", "pub"),
    "pane_idx": ("pane", "pub(crate)"),
    "parse_path": ("nav", "pub(crate)"),
    "next_sort_key": ("nav", "pub(crate)"),
    "sort_label": ("nav", "pub(crate)"),
    "recursive_status": ("attrs", "pub(crate)"),
    "attr_status": ("attrs", "pub(crate)"),
    "RECURSE_NODE_CAP": ("attrs", "pub(crate)"),
    "resume_offer_view": ("jobs", "pub(crate)"),
    "job_status_from": ("jobs", "pub(crate)"),
    "crc32_partial": ("jobs", "pub(crate)"),
}

# Test fns: helpers -> test_support; the rest route by name.
TEST_SUPPORT_HELPERS = {
    "make_app", "mode_of", "entry_index", "app_with_three", "submit_one_copy",
    "make_compare_app", "make_nested_app", "wait_status", "stage_checkpoint",
    "wait_completed", "file_uri",
}
TEST_MODULE = {
    # attrs
    "collect_subtree_enumerates_depth_first_to_last": "attrs",
    "collect_subtree_does_not_follow_symlinked_dir": "attrs",
    "collect_subtree_file_root_is_only_itself": "attrs",
    "collect_subtree_capped_truncates": "attrs",
    "chown_recursive_noop_to_current_owner_at_depth": "attrs",
    "chown_recursive_unknown_owner_does_not_walk": "attrs",
    "chown_recursive_empty_selection_is_noop": "attrs",
    "chmod_recursive_applies_at_depth": "attrs",
    "chmod_recursive_symbolic_is_per_entry": "attrs",
    "chmod_recursive_deepest_first_no_lockout": "attrs",
    "chmod_recursive_does_not_follow_symlink": "attrs",
    "chmod_recursive_invalid_does_not_walk": "attrs",
    "chmod_recursive_file_only_is_shallow": "attrs",
    "chmod_selection_sets_focused_file": "attrs",
    "chmod_selection_symbolic_and_multi_file": "attrs",
    "chmod_selection_invalid_changes_nothing": "attrs",
    "chmod_selection_partial_failure_reports_and_continues": "attrs",
    "chmod_selection_on_parent_row_is_noop": "attrs",
    "chown_selection_noop_to_current_owner_ok": "attrs",
    "chown_selection_group_only_numeric_ok": "attrs",
    "chown_selection_unknown_user_is_bad_attr": "attrs",
    "chown_selection_empty_is_bad_attr": "attrs",
    "create_symlink_points_at_focused_entry": "attrs",
    "create_symlink_existing_name_is_refused": "attrs",
    "create_hard_link_shares_content": "attrs",
    "create_hard_link_to_directory_errors": "attrs",
    "create_symlink_blank_name_is_bad_attr": "attrs",
    # hotlist
    "add_bookmark_uses_active_cwd_and_persists": "hotlist",
    "add_bookmark_rejects_blank_name": "hotlist",
    "add_bookmark_allows_duplicate_names": "hotlist",
    "bookmarks_persist_and_reload": "hotlist",
    "remove_bookmark_drops_and_persists": "hotlist",
    "jump_to_missing_target_is_graceful_and_retains_bookmark": "hotlist",
    "jump_to_bookmark_navigates_active_pane": "hotlist",
    # app
    "new_loads_both_pane_listings": "app",
    "new_rejects_relative_path": "app",
    "cursor_down_advances_within_visible_subset": "app",
    "cursor_to_sets_absolute_position_and_clamps": "app",
    "cursor_to_survives_resync_via_pane_state": "app",
    "focus_swap_toggles_active_pane": "app",
    "cycle_listing_mode_rotates_view": "app",
    "quit_emits_quit_requested": "app",
    "toggle_hidden_resets_cursor": "app",
    "toggle_split_orientation_cycles_horizontal_vertical": "app",
    "pane_accessor_returns_starting_cwd": "app",
    "active_pane_state_returns_active_side": "app",
    "app_registry_returns_arc_vfs_registry": "app",
    "pane_backend_is_local_fs_on_startup": "app",
    # pane
    "root_pane_has_no_parent_row": "pane",
    "non_root_pane_has_parent_row_and_offset": "pane",
    "default_cursor_is_first_real_entry": "pane",
    "cursor_up_from_first_entry_lands_on_parent_then_clamps": "pane",
    "descend_on_parent_row_ascends": "pane",
    "selection_toggle_on_parent_row_is_noop": "pane",
    "selection_invert_and_pattern_exclude_parent_row": "pane",
    "copy_on_parent_row_with_no_selection_targets_nothing": "pane",
    "parent_row_present_regardless_of_filter": "pane",
    "empty_non_root_dir_focuses_parent_row": "pane",
    "selection_toggle_marks_focused_entry": "pane",
    "glob_match_basic": "pane",
    "pane_filter_glob_matches_extension": "pane",
    "pane_filter_bare_word_matches_as_substring": "pane",
    "pane_filter_is_case_insensitive": "pane",
    "pane_filter_invalid_pattern_errors": "pane",
    "pane_filter_pattern_accessor_returns_trimmed_original": "pane",
    "pane_state_has_backend_field": "pane",
    # nav
    "descend_into_subdir_then_ascend_back": "nav",
    "cycle_sort_key_reorders_listing": "nav",
    "toggle_sort_reverse_flips_name_order": "nav",
    "set_filter_applies_glob_and_resets_cursor": "nav",
    "set_filter_bare_word_is_substring": "nav",
    "set_filter_only_affects_active_pane": "nav",
    "set_filter_persists_across_navigation": "nav",
    "set_filter_empty_clears_existing_filter": "nav",
    "set_filter_whitespace_clears_and_noop_when_none": "nav",
    "set_filter_invalid_pattern_leaves_pane_unchanged": "nav",
    "toggle_panel_filter_dispatch_is_noop_in_core": "nav",
    "sync_other_panel_path_copies_other_pane_cwd_into_active": "nav",
    "show_focused_in_other_panel_navigates_other_pane": "nav",
    "show_focused_in_other_panel_is_noop_on_file": "nav",
    "quick_cd_popup_dispatch_is_noop_in_core": "nav",
    "quick_cd_absolute_path_navigates_active_pane": "nav",
    "quick_cd_relative_path_resolves_against_cwd": "nav",
    "quick_cd_dotdot_ascends": "nav",
    "quick_cd_trailing_slash_ignored": "nav",
    "quick_cd_records_history": "nav",
    "quick_cd_empty_is_noop": "nav",
    "quick_cd_nonexistent_errors_without_navigating": "nav",
    "quick_cd_file_target_errors_without_navigating": "nav",
    "complete_cd_unique_prefix_single_candidate": "nav",
    "complete_cd_multiple_matches_in_sort_order": "nav",
    "complete_cd_excludes_files": "nav",
    "complete_cd_recent_dir_ordered_first": "nav",
    "complete_cd_no_match_is_empty": "nav",
    "quick_cd_end_to_end_complete_accept_cancel_and_recover": "nav",
    "ascend_from_zip_root_returns_to_local_parent": "nav",
    # history
    "descend_pushes_back_history_clears_forward": "history",
    "history_prev_dir_pops_back_pushes_to_forward": "history",
    "history_next_dir_returns_after_prev": "history",
    "history_prev_dir_with_empty_history_is_noop_with_status": "history",
    "descend_after_prev_drops_forward_history": "history",
    # fsops
    "mkdir_creates_directory_and_refreshes": "fsops",
    "mkdir_rejects_invalid_name_without_crash": "fsops",
    "select_by_pattern_tags_matches_and_unselect_removes": "fsops",
    "select_by_pattern_zero_match_reports_zero": "fsops",
    "recursive_dir_size_sums_tree": "fsops",
    # compare
    "compare_left_only_tags_left_pane_only": "compare",
    "compare_right_only_tags_right_pane_only": "compare",
    "compare_size_differ_tags_both_panes": "compare",
    "compare_hash_differ_tags_both_panes": "compare",
    "compare_identical_entries_not_tagged": "compare",
    "compare_same_path_both_panels_returns_status_no_tags": "compare",
    "compare_additive_does_not_clear_existing_selection": "compare",
    "compare_large_dir_emits_status_comparing_first": "compare",
    # jobs (crc32)
    "crc32_partial_same_content_same_hash": "jobs",
    "crc32_partial_different_content_different_hash": "jobs",
    "crc32_partial_large_file_uses_head_only": "jobs",
    "crc32_partial_unreadable_path_returns_none": "jobs",
    "crc32_partial_empty_file_consistent_hash": "jobs",
    # rename
    "apply_bulk_rename_empty_pairs_returns_no_changes": "rename",
    "apply_bulk_rename_two_of_three_renamed_on_disk": "rename",
    "apply_bulk_rename_collision_no_renames_applied": "rename",
    "apply_bulk_rename_returns_pane_updated_and_status_events": "rename",
    "apply_bulk_rename_records_undo_entry_reversed": "rename",
    "apply_bulk_rename_partial_failure_records_partial_undo": "rename",
    "apply_bulk_rename_second_call_overwrites_undo_log": "rename",
    "undo_none_log_returns_nothing_to_undo": "rename",
    "undo_rename_restores_files_on_disk": "rename",
    "undo_copy_deletes_destination_copies": "rename",
    "undo_delete_returns_cannot_be_undone": "rename",
    "undo_second_call_returns_nothing_to_undo": "rename",
    "undo_clears_selection_on_both_panes": "rename",
    "undo_move_scaffold_does_not_crash": "rename",
    "validate_rename_all_unchanged_returns_empty": "rename",
    "validate_rename_two_of_three_changed_correct_pairs": "rename",
    "validate_rename_line_count_mismatch_returns_err": "rename",
    "validate_rename_empty_name_returns_err": "rename",
    "validate_rename_slash_in_name_returns_err": "rename",
    "validate_rename_duplicate_proposed_names_returns_err": "rename",
    "validate_rename_correct_output_pairs_ordering": "rename",
    # transfers
    "submit_one_copy": "transfers",  # (also in helpers; helpers win)
    "copy_with_no_selection_emits_status_not_dialog": "transfers",
    "copy_with_selection_requests_confirmation": "transfers",
    "confirm_copy_spawns_a_transfer": "transfers",
    "show_tasks_panel_dispatch_is_noop": "transfers",
    "job_views_empty_when_no_transfers": "transfers",
    "job_views_lists_transfers_in_submit_order": "transfers",
    "cancel_transfer_signals_only_that_job": "transfers",
    "cancel_transfer_unknown_id_is_safe_noop": "transfers",
    "pause_transfer_marks_paused_and_cancels_token": "transfers",
    "pause_transfer_unknown_id_is_safe_noop": "transfers",
    "resume_paused_noop_when_not_paused": "transfers",
    "cancel_transfer_clears_paused_marker": "transfers",
    "three_jobs_pause_one_others_continue": "transfers",
    "cancel_current_transfer_signals_cancel_on_latest": "transfers",
    "pending_resume_views_empty_on_fresh_app": "transfers",
    "scan_finds_offer_in_a_pane_dir": "transfers",
    "scan_finds_nothing_when_no_sidecars": "transfers",
    "scan_ignores_malformed_sidecar": "transfers",
    "resume_offer_completes_and_matches_source": "transfers",
    "resume_offer_fails_safe_on_changed_destination": "transfers",
    "start_over_discards_checkpoint_and_copies_fresh": "transfers",
    "skip_offer_starts_nothing_and_keeps_sidecar": "transfers",
    # tabs
    "side_state_struct_shape": "tabs",
    "tab_bar_entry_struct_shape": "tabs",
    "tab_new_dispatch_returns_ok": "tabs",
    "tab_close_dispatch_returns_ok": "tabs",
    "tab_next_dispatch_returns_ok": "tabs",
    "tab_prev_dispatch_returns_ok": "tabs",
    "tab_new_opens_in_same_cwd": "tabs",
    "tab_new_inherits_no_filter_or_selection": "tabs",
    "tab_new_becomes_active": "tabs",
    "tab_close_noop_on_single_tab": "tabs",
    "tab_close_selects_right_successor": "tabs",
    "tab_close_wraps_to_last_when_rightmost": "tabs",
    "tab_next_advances_and_wraps": "tabs",
    "tab_prev_recedes_and_wraps": "tabs",
    "tab_next_noop_with_one_tab": "tabs",
    "cross_pane_copy_dest_is_active_tab_cwd": "tabs",
    "cross_pane_copy_after_tab_switch_uses_new_active": "tabs",
    "sync_other_panel_uses_active_tab_cwd": "tabs",
    "dialog_dest_captured_at_open_time": "tabs",
    "tab_state_filter_is_isolated": "tabs",
    "tab_state_sort_is_isolated": "tabs",
    "tab_state_selection_is_isolated": "tabs",
    "tab_new_does_not_inherit_filter": "tabs",
    "tab_state_show_hidden_is_isolated": "tabs",
    "tab_state_history_is_isolated": "tabs",
    "focus_swap_key_does_not_change_tabs": "tabs",
    "tab_bar_view_single_tab": "tabs",
    "tab_bar_view_multiple_tabs": "tabs",
    "tab_bar_view_label_truncates_long_name": "tabs",
    "tab_bar_view_active_marker_on_correct_tab": "tabs",
}

PROD_MODULES = ["pane", "command", "error", "jobs", "app", "nav", "history",
                "fsops", "attrs", "compare", "rename", "hotlist", "tabs",
                "transfers"]


# --- tokenizer-aware span scanner ----------------------------------------

def scan_spans(text):
    """Yield (kind, name, start_char, end_char) for depth-0 constructs and,
    for `impl App` and `mod tests`, descend to depth-1 items.
    Returns a flat structure we post-process by lines."""
    # We work on characters but map back to lines later.
    return text


def strip_comments_map(text):
    """Return a parallel string where bytes inside comments/strings/chars are
    replaced by spaces (newlines preserved) so brace/keyword scanning is safe."""
    out = []
    i, n = 0, len(text)
    state = None  # None, 'line', 'block', 'str', 'char', 'raw'
    block_depth = 0
    raw_hashes = 0
    while i < n:
        c = text[i]
        nxt = text[i + 1] if i + 1 < n else ""
        if state is None:
            if c == "/" and nxt == "/":
                state = "line"; out.append("  "); i += 2; continue
            if c == "/" and nxt == "*":
                state = "block"; block_depth = 1; out.append("  "); i += 2; continue
            if c == "r" and (nxt == '"' or (nxt == "#")):
                # raw string r"..." or r#"..."#
                j = i + 1; hashes = 0
                while j < n and text[j] == "#":
                    hashes += 1; j += 1
                if j < n and text[j] == '"':
                    state = "raw"; raw_hashes = hashes
                    out.append(" " * (j - i + 1)); i = j + 1; continue
                out.append(c); i += 1; continue
            if c == '"':
                state = "str"; out.append(" "); i += 1; continue
            if c == "'":
                # char literal or lifetime; treat 'x' / '\n' / '\u{..}' as char
                m = re.match(r"'(\\.|\\u\{[0-9A-Fa-f]+\}|[^'\\])'", text[i:])
                if m:
                    out.append(" " * len(m.group(0))); i += len(m.group(0)); continue
                out.append(c); i += 1; continue
            out.append(c); i += 1; continue
        if state == "line":
            if c == "\n":
                state = None; out.append("\n")
            else:
                out.append(" ")
            i += 1; continue
        if state == "block":
            if c == "/" and nxt == "*":
                block_depth += 1; out.append("  "); i += 2; continue
            if c == "*" and nxt == "/":
                block_depth -= 1; out.append("  "); i += 2
                if block_depth == 0:
                    state = None
                continue
            out.append("\n" if c == "\n" else " "); i += 1; continue
        if state == "str":
            if c == "\\":
                out.append("  "); i += 2; continue
            if c == '"':
                state = None; out.append(" "); i += 1; continue
            out.append("\n" if c == "\n" else " "); i += 1; continue
        if state == "raw":
            if c == '"':
                j = i + 1; hashes = 0
                while j < n and text[j] == "#" and hashes < raw_hashes:
                    hashes += 1; j += 1
                if hashes == raw_hashes:
                    state = None; out.append(" " * (j - i)); i = j; continue
                out.append(" "); i += 1; continue
            out.append("\n" if c == "\n" else " "); i += 1; continue
    return "".join(out)


def line_starts(text):
    starts = [0]
    for i, c in enumerate(text):
        if c == "\n":
            starts.append(i + 1)
    return starts


def char_to_line(pos, starts):
    # binary search
    lo, hi = 0, len(starts) - 1
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if starts[mid] <= pos:
            lo = mid
        else:
            hi = mid - 1
    return lo  # 0-based line index


def main():
    text = open(SRC).read()
    masked = strip_comments_map(text)
    assert len(masked) == len(text)
    lines = text.split("\n")
    starts = line_starts(text)

    # Find depth-0 brace matching to locate constructs.
    # Strategy: iterate lines; at depth 0, a line whose masked content matches a
    # construct keyword begins a construct; find its end by brace/semicolon.
    construct_re = re.compile(
        r"^\s*(?:pub\s*(?:\([^)]*\))?\s+)?"
        r"(?:async\s+)?(?:unsafe\s+)?"
        r"(impl|struct|enum|trait|fn|const|static|type|mod|use|pub use)\b")

    masked_lines = masked.split("\n")

    def depth_after(idx):
        return masked_lines[idx].count("{") - masked_lines[idx].count("}")

    # Build depth at start of each line.
    depth = [0] * (len(masked_lines) + 1)
    for i in range(len(masked_lines)):
        depth[i + 1] = depth[i] + depth_after(i)

    # Identify top-level constructs (depth at line start == 0 and matches).
    constructs = []  # (start_line_idx, end_line_idx, header_text)
    i = 0
    N = len(masked_lines)
    while i < N:
        if depth[i] == 0 and construct_re.match(masked_lines[i]) and not masked_lines[i].lstrip().startswith("}"):
            # find end: scan until braces balance (if any) or semicolon at depth0
            start = i
            # does this construct use braces or end with ; ?
            j = i
            opened = False
            d = 0
            end = i
            while j < N:
                d += masked_lines[j].count("{") - masked_lines[j].count("}")
                if "{" in masked_lines[j]:
                    opened = True
                if opened and d == 0:
                    end = j; break
                if not opened and masked_lines[j].rstrip().endswith(";"):
                    end = j; break
                j += 1
            else:
                end = N - 1
            constructs.append([start, end])
            i = end + 1
        else:
            i += 1

    # Attach leading doc/attr lines to each construct.
    def extend_up(start):
        s = start
        while s - 1 >= 0:
            t = lines[s - 1].strip()
            if t.startswith("///") or t.startswith("#[") or t.startswith("#!["):
                s -= 1
            elif t.endswith("]") and ("#[" in "".join(lines[max(0, s - 3):s])):
                s -= 1
            else:
                break
        return s

    # classify constructs
    name_re = {
        "impl": re.compile(r"\bimpl\b[^{]*?\bfor\b\s+([A-Za-z_][A-Za-z0-9_]*)|\bimpl\s+([A-Za-z_][A-Za-z0-9_]*)"),
        "struct": re.compile(r"\bstruct\s+([A-Za-z_][A-Za-z0-9_]*)"),
        "enum": re.compile(r"\benum\s+([A-Za-z_][A-Za-z0-9_]*)"),
        "trait": re.compile(r"\btrait\s+([A-Za-z_][A-Za-z0-9_]*)"),
        "fn": re.compile(r"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)"),
        "const": re.compile(r"\bconst\s+([A-Za-z_][A-Za-z0-9_]*)"),
        "type": re.compile(r"\btype\s+([A-Za-z_][A-Za-z0-9_]*)"),
    }

    # Buckets
    files = {m: [] for m in PROD_MODULES}
    impl_methods = {m: [] for m in PROD_MODULES}
    test_blocks = {m: [] for m in PROD_MODULES}
    test_support = []
    lib_keep = []  # constructs staying in lib.rs (App, SideState)
    header_use_lines = []  # converted pub(crate) use

    def get_kw(headline):
        m = construct_re.match(masked_lines[headline])
        return m.group(1) if m else None

    def slice_text(a, b):
        return "\n".join(lines[a:b + 1])

    handled = set()
    for (cs, ce) in constructs:
        kw = get_kw(cs)
        full_start = extend_up(cs)
        block = slice_text(full_start, ce)
        headline = lines[cs]
        if kw in ("use", "pub use"):
            continue  # handled separately for header
        if kw == "impl":
            mm = name_re["impl"].search(masked_lines[cs])
            tname = (mm.group(1) or mm.group(2)) if mm else None
            if tname == "App":
                # descend into methods
                route_impl_app(lines, masked_lines, cs, ce, impl_methods, test_support)
                handled.add((cs, ce))
                continue
            else:
                mod = TYPE_MODULE.get(tname)
                if mod:
                    files[mod].append(block)
                    handled.add((cs, ce))
                    continue
                else:
                    raise SystemExit(f"unrouted impl for {tname} at line {cs+1}")
        if kw in ("struct", "enum", "trait", "type"):
            mm = name_re[kw].search(masked_lines[cs])
            nm = mm.group(1) if mm else None
            if nm in ("App", "SideState"):
                lib_keep.append(block); handled.add((cs, ce)); continue
            mod = TYPE_MODULE.get(nm)
            if mod is None:
                raise SystemExit(f"unrouted {kw} {nm} at line {cs+1}")
            files[mod].append(block); handled.add((cs, ce)); continue
        if kw in ("fn", "const", "static"):
            mm = name_re["fn"].search(masked_lines[cs]) or name_re["const"].search(masked_lines[cs])
            nm = mm.group(1) if mm else None
            if nm in FREEFN_MODULE:
                mod, vis = FREEFN_MODULE[nm]
                files[mod].append(apply_vis(block, vis))
                handled.add((cs, ce)); continue
            raise SystemExit(f"unrouted free fn/const {nm} at line {cs+1}")
        if kw == "mod":
            # the test module
            if "mod tests" in masked_lines[cs]:
                route_tests(lines, masked_lines, cs, ce, test_blocks, test_support)
                handled.add((cs, ce)); continue
            raise SystemExit(f"unexpected mod at line {cs+1}: {headline}")

    write_outputs(files, impl_methods, test_blocks, test_support, lib_keep, text)


def apply_vis(block, vis):
    """Ensure the item has the requested visibility (pub / pub(crate))."""
    lines = block.split("\n")
    for i, ln in enumerate(lines):
        s = ln.lstrip()
        if s.startswith("///") or s.startswith("#["):
            continue
        # first real code line
        indent = ln[:len(ln) - len(s)]
        if s.startswith("pub(crate)") or s.startswith("pub "):
            # normalize to requested vis if it's currently bare pub vs pub(crate)
            if vis == "pub(crate)" and s.startswith("pub ") and not s.startswith("pub("):
                lines[i] = indent + "pub(crate) " + s[len("pub "):]
            return "\n".join(lines)
        if vis and not (s.startswith("pub")):
            lines[i] = indent + vis + " " + s
        return "\n".join(lines)
    return block


def collect_methods(lines, masked_lines, cs, ce):
    """Return list of (name, start_line, end_line) for fns directly inside the
    impl/mod block spanning [cs, ce] (header at cs, closing brace at ce)."""
    fn_re = re.compile(r"^\s+(?:pub\s*(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)")
    const_re = re.compile(r"^\s+(?:pub\s*(?:\([^)]*\))?\s+)?const\s+([A-Za-z_][A-Za-z0-9_]*)")
    items = []
    i = cs + 1
    # compute relative depth within block; body lines are at depth>=1
    d = 0
    # recompute depth from cs
    base = 0
    for k in range(cs, ce + 1):
        line_open = masked_lines[k].count("{")
        line_close = masked_lines[k].count("}")
        if k == cs:
            base += line_open - line_close
            continue
        # at this point 'base' is depth at start of line k
        if base == 1:
            m = fn_re.match(masked_lines[k]) or const_re.match(masked_lines[k])
            if m:
                # find end of this fn/const
                name = m.group(1)
                j = k; dd = 0; opened = False; endk = k
                while j <= ce:
                    dd += masked_lines[j].count("{") - masked_lines[j].count("}")
                    if "{" in masked_lines[j]:
                        opened = True
                    if opened and dd == 0:
                        endk = j; break
                    if not opened and masked_lines[j].rstrip().endswith(";"):
                        endk = j; break
                    j += 1
                items.append((name, k, endk))
        base += line_open - line_close
    return items


def extend_up_lines(lines, start):
    s = start
    while s - 1 >= 0:
        t = lines[s - 1].strip()
        if t.startswith("///") or t.startswith("#[") or t.startswith("#!["):
            s -= 1
        else:
            break
    return s


def route_impl_app(lines, masked_lines, cs, ce, impl_methods, test_support):
    methods = collect_methods(lines, masked_lines, cs, ce)
    for (name, ms, me) in methods:
        full = extend_up_lines(lines, ms)
        block = "\n".join(lines[full:me + 1])
        mod = METHOD_MODULE.get(name)
        if mod is None:
            raise SystemExit(f"unrouted impl App method {name} at line {ms+1}")
        # widen private methods to pub(crate) so cross-module dispatch works;
        # leave already-pub (public API) untouched.
        block = widen_method(block)
        impl_methods[mod].append(block)


def widen_method(block):
    lines = block.split("\n")
    for i, ln in enumerate(lines):
        s = ln.lstrip()
        if s.startswith("///") or s.startswith("#["):
            continue
        indent = ln[:len(ln) - len(s)]
        if s.startswith("pub"):
            return "\n".join(lines)  # keep public API as-is
        # bare fn -> pub(crate)
        lines[i] = indent + "pub(crate) " + s
        return "\n".join(lines)
    return block


def route_tests(lines, masked_lines, cs, ce, test_blocks, test_support):
    items = collect_methods(lines, masked_lines, cs, ce)
    for (name, ms, me) in items:
        full = extend_up_lines(lines, ms)
        block = "\n".join(lines[full:me + 1])
        if name in TEST_SUPPORT_HELPERS:
            test_support.append(widen_helper(block))
        else:
            mod = TEST_MODULE.get(name)
            if mod is None:
                raise SystemExit(f"unrouted test {name} at line {ms+1}")
            test_blocks[mod].append(block)


def widen_helper(block):
    """Make a test helper pub(crate) so co-located test modules can call it."""
    lines = block.split("\n")
    for i, ln in enumerate(lines):
        s = ln.lstrip()
        if s.startswith("///") or s.startswith("#["):
            continue
        indent = ln[:len(ln) - len(s)]
        if s.startswith("pub"):
            return "\n".join(lines)
        lines[i] = indent + "pub(crate) " + s
        return "\n".join(lines)
    return block


HEADER = """// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
"""


def write_outputs(files, impl_methods, test_blocks, test_support, lib_keep, original):
    # Dedent helper blocks (they were indented inside `mod tests`).
    def dedent(block):
        out = []
        for ln in block.split("\n"):
            if ln.startswith("    "):
                out.append(ln[4:])
            else:
                out.append(ln)
        return "\n".join(out)

    # Write each production module.
    for mod in PROD_MODULES:
        body = []
        body.append("// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.")
        body.append("// SPDX-License-Identifier: MIT OR Apache-2.0")
        body.append("")
        body.append(f"//! Feature 059 split: `{mod}` module of `cargonaut-core`.")
        body.append("//!")
        body.append("//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).")
        body.append("")
        body.append("#[allow(unused_imports)]")
        body.append("use crate::*;")
        body.append("")
        for blk in files[mod]:
            body.append(blk)
            body.append("")
        if impl_methods[mod]:
            body.append("impl App {")
            for blk in impl_methods[mod]:
                body.append(blk)
                body.append("")
            body.append("}")
            body.append("")
        if test_blocks[mod]:
            body.append("#[cfg(test)]")
            body.append("mod tests {")
            body.append("    use super::*;")
            body.append("    #[allow(unused_imports)]")
            body.append("    use crate::test_support::*;")
            body.append("")
            for blk in test_blocks[mod]:
                body.append(blk)
                body.append("")
            body.append("}")
        open(f"{OUT_DIR}/{mod}.rs", "w").write("\n".join(body) + "\n")

    # test_support.rs
    ts = []
    ts.append("// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.")
    ts.append("// SPDX-License-Identifier: MIT OR Apache-2.0")
    ts.append("")
    ts.append("//! Shared unit-test fixtures (Feature 059 split). `#[cfg(test)]` only.")
    ts.append("")
    ts.append("#[allow(unused_imports)]")
    ts.append("pub(crate) use crate::*;")
    ts.append("pub(crate) use cargonaut_vfs::VfsCaps;")
    ts.append("pub(crate) use tempfile::TempDir;")
    ts.append("#[allow(unused_imports)]")
    ts.append("pub(crate) use tokio::fs;")
    ts.append("#[allow(unused_imports)]")
    ts.append("pub(crate) use sha2::{Digest, Sha256};")
    ts.append("")
    for blk in test_support:
        ts.append(dedent(blk))
        ts.append("")
    open(f"{OUT_DIR}/test_support.rs", "w").write("\n".join(ts) + "\n")

    # Rebuild lib.rs: original lines 1..(App struct region) but we reconstruct.
    # Take original header/doc/use block up to first construct we moved.
    orig_lines = original.split("\n")
    # crate docs + use block = lines before the first "// ====" banner at line ~39
    # We'll grab everything up to the PaneId banner (line index 38 -> "// ===").
    # Simpler: keep lines [0 .. first 'pub enum PaneId') minus nothing, convert use.
    head = []
    for ln in orig_lines:
        if ln.startswith("pub enum PaneId"):
            break
        head.append(ln)
    # drop trailing banner / doc / attribute lines that belong to PaneId
    while head and (head[-1].lstrip().startswith("//")
                    or head[-1].lstrip().startswith("#[")
                    or head[-1].strip() == ""):
        head.pop()
    # convert private `use` to `pub(crate) use` so submodules see them via glob
    newhead = []
    for ln in head:
        st = ln.lstrip()
        if st.startswith("use ") and not st.startswith("use crate"):
            indent = ln[:len(ln) - len(st)]
            newhead.append(indent + "pub(crate) " + st)
        else:
            newhead.append(ln)
    lib = []
    lib.extend(newhead)
    lib.append("")
    # module declarations
    lib.append("// Feature 059 — implementation split into cohesive submodules.")
    for mod in ["pane", "command", "error", "jobs", "app", "nav", "history",
                "fsops", "attrs", "compare", "rename", "hotlist", "tabs",
                "transfers"]:
        lib.append(f"mod {mod};")
    lib.append("#[cfg(test)]")
    lib.append("mod test_support;")
    lib.append("")
    # public re-export surface
    lib.append("pub use command::{Command, DialogKind, Event};")
    lib.append("pub use error::{AppError, UndoEntry};")
    lib.append("pub use jobs::{")
    lib.append("    transfer_state_snapshot, JobStatus, JobView, ProgressView, ResumeOfferView,")
    lib.append("};")
    lib.append("pub use pane::{")
    lib.append("    glob_match, FocusedRow, PaneFilter, PaneId, PaneState, SplitOrient, TabBarEntry,")
    lib.append("    ViewMode,")
    lib.append("};")
    lib.append("pub use rename::validate_rename_proposals;")
    lib.append("")
    # crate-internal helper re-exports so `use crate::*` finds cross-module fns
    lib.append("#[allow(unused_imports)]")
    lib.append("pub(crate) use pane::pane_idx;")
    lib.append("#[allow(unused_imports)]")
    lib.append("pub(crate) use nav::{next_sort_key, parse_path, sort_label};")
    lib.append("#[allow(unused_imports)]")
    lib.append("pub(crate) use attrs::{attr_status, recursive_status, RECURSE_NODE_CAP};")
    lib.append("#[allow(unused_imports)]")
    lib.append("pub(crate) use jobs::{crc32_partial, job_status_from, resume_offer_view};")
    lib.append("")
    # App + SideState definitions (kept at root)
    for blk in lib_keep:
        lib.append(blk)
        lib.append("")
    open(f"{OUT_DIR}/lib.rs", "w").write("\n".join(lib) + "\n")

    # report
    print("== module line counts ==")
    import os
    total = 0
    for mod in PROD_MODULES + ["test_support", "lib"]:
        p = f"{OUT_DIR}/{mod}.rs"
        if os.path.exists(p):
            c = sum(1 for _ in open(p))
            total += c
            print(f"{c:5d}  {mod}.rs")
    print(f"total {total} (orig 6246)")


if __name__ == "__main__":
    main()

use interpreter::interpreter::Interpreter;
use core::ast::ArtValue;

#[test]
fn arena_objects_finalized_counter() {
    let mut interp = Interpreter::new();
    // create an arena id and register two heap composites inside it
    let aid = 42u32;
    let _id1 = interp.debug_heap_register_in_arena(ArtValue::Int(1), aid);
    let _id2 = interp.debug_heap_register_in_arena(ArtValue::Int(2), aid);
    // run finalization for the arena
    interp.debug_finalize_arena(aid);
    // objects_finalized_per_arena should record 2 finalized objects for this arena
    let cnt = interp.objects_finalized_per_arena.get(&aid).cloned().unwrap_or(0);
    assert_eq!(cnt, 2, "expected 2 finalized objects for arena {} but got {}", aid, cnt);
}

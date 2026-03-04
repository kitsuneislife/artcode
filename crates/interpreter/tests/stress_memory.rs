use core::ast::{ArtValue, ObjHandle};
use interpreter::interpreter::Interpreter;

#[test]
fn test_memory_arena_massive_wide_cycle_stress() {
    let builder = std::thread::Builder::new().stack_size(32 * 1024 * 1024);
    let handler = builder
        .spawn(|| {
            let mut interp = Interpreter::with_prelude();

            let runs = 20;
            let wide_spread = 500;

            for _ in 0..runs {
                let arena_id = interp.debug_create_arena();

                // Setup a "Global Manager" node in this arena
                let root_handle =
                    interp.debug_heap_register_in_arena(ArtValue::Array(vec![]), arena_id);

                for i in 1..wide_spread {
                    // Creates a new peer node: [id, root] (root creates the cycle back up)
                    let peer_node = interp.debug_heap_register_in_arena(
                        ArtValue::Array(vec![
                            ArtValue::Int(i),
                            ArtValue::HeapComposite(ObjHandle(root_handle)),
                        ]),
                        arena_id,
                    );

                    // Root registers the peer downward (bidirectional strong cycle link)
                    if let Some(mut val) = interp.debug_heap_get_unowned(root_handle) {
                        if let ArtValue::Array(ref mut a) = val {
                            a.push(ArtValue::HeapComposite(ObjHandle(peer_node)));
                        }
                        interp.debug_heap_set(root_handle, val);
                    }
                }

                // At this point we have a massive star-shaped cycle:
                // 1 Root -> 500 Nodes. 500 Nodes -> 1 Root.
                interp.debug_finalize_arena(arena_id);
            }

            println!(
                "Cycle leaks recovered by Arenas: {}",
                interp.cycle_leaks_detected
            );

            // Finalized objects must be greater than total allocations!
            // 20 runs * 500 nodes (+ 1 root) = ~10.000 allocations total garbage collected automatically natively.
            assert!(
                interp.objects_finalized >= 10_000,
                "Arena GC is not clearing the massive cycle cluster memory stress!"
            );
        })
        .unwrap();

    handler.join().unwrap();
}

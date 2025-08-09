use interpreter::interpreter::Interpreter;
use core::ast::ArtValue;

#[test]
fn simple_array_cycle_detection() {
    // Criar dois arrays A e B que se referenciam formando ciclo composto
    // (Simples: Array contendo outro Array; não temos refs diretas ainda, então simulamos estrutura aninhada repetida)
    // Como o grafo usa endereços de ArtValue compostos, repetição de inserção não cria dois nós diferentes para mesmo objeto.
    // Para forçar ciclo precisamos estruturas mutuamente referenciadas: ainda não suportado sem referências internas reais.
    // Teste mínimo: detect_cycles não deve panic e retorna estrutura consistente.
    let mut interp = Interpreter::with_prelude();
    // Apenas registra alguns valores compostos
    let arr1 = ArtValue::Array(vec![ArtValue::Int(1)]);
    let arr2 = ArtValue::Array(vec![ArtValue::Int(2)]);
    interp.debug_heap_register(arr1.clone());
    interp.debug_heap_register(arr2.clone());
    // Coloca no ambiente para serem roots
    interp.debug_define_global("a", arr1);
    interp.debug_define_global("b", arr2);
    let res = interp.detect_cycles();
    // Sem ciclo real esperado
    assert!(res.cycles.is_empty());
}

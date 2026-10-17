use weftora_api::{StateError, Value};
use weftora_core::State;
use weftora_flow::{CompareOp::*, Condition, ConditionError, MAX_DEPTH, MAX_NODES, Operand};

fn comparison(op: weftora_flow::CompareOp, left: Value, right: Value) -> Condition {
    Condition::Compare {
        op,
        left: Operand::Literal(left),
        right: Operand::Literal(right),
    }
}

fn boolean(value: bool) -> Condition {
    comparison(Eq, Value::Bool(value), Value::Bool(true))
}

#[test]
fn all_comparisons_on_ordered_scalars() {
    let pairs = [
        (Value::Int(i64::MIN), Value::Int(i64::MAX)),
        (Value::Int(i64::MAX - 1), Value::Int(i64::MAX)),
        (Value::Float(-1.5), Value::Float(2.5)),
        (Value::String("a".into()), Value::String("é".into())),
    ];
    for (left, right) in pairs {
        for (a, b, expected) in [
            (&left, &right, [false, true, true, true, false, false]),
            (&right, &left, [false, true, false, false, true, true]),
            (&left, &left, [true, false, false, true, false, true]),
        ] {
            for (op, expected) in [Eq, Ne, Lt, Le, Gt, Ge].into_iter().zip(expected) {
                assert_eq!(
                    comparison(op, a.clone(), b.clone()).evaluate(&State::new()),
                    Ok(expected),
                    "{a:?} {op:?} {b:?}"
                );
            }
        }
    }
    assert_eq!(
        comparison(Eq, Value::Float(-0.0), Value::Float(0.0)).evaluate(&State::new()),
        Ok(true)
    );
}

#[test]
fn boolean_comparisons_and_invalid_type_pairs() {
    let state = State::new();
    for a in [false, true] {
        for b in [false, true] {
            for (op, expected) in [(Eq, a == b), (Ne, a != b)] {
                assert_eq!(
                    comparison(op, Value::Bool(a), Value::Bool(b)).evaluate(&state),
                    Ok(expected)
                );
            }
            for op in [Lt, Le, Gt, Ge] {
                assert!(matches!(
                    comparison(op, Value::Bool(a), Value::Bool(b)).validate(&state),
                    Err(ConditionError::Operands { .. })
                ));
            }
        }
    }
    let values = [
        Value::Bool(true),
        Value::Int(1),
        Value::Float(1.0),
        Value::String("1".into()),
    ];
    for (i, left) in values.iter().enumerate() {
        for (j, right) in values.iter().enumerate() {
            if i == j {
                continue;
            }
            for op in [Eq, Ne, Lt, Le, Gt, Ge] {
                assert!(matches!(
                    comparison(op, left.clone(), right.clone()).evaluate(&state),
                    Err(ConditionError::Operands { .. })
                ));
            }
        }
    }
}

#[test]
fn boolean_operators_and_validation_of_skipped_branches() {
    let state = State::new();
    let nested = Condition::Not(Box::new(Condition::And(vec![
        Condition::Or(vec![boolean(false), boolean(true)]),
        boolean(false),
    ])));
    assert_eq!(nested.evaluate(&state), Ok(true));
    for a in [false, true] {
        for b in [false, true] {
            assert_eq!(
                Condition::And(vec![boolean(a), boolean(b)]).evaluate(&state),
                Ok(a && b)
            );
            assert_eq!(
                Condition::Or(vec![boolean(a), boolean(b)]).evaluate(&state),
                Ok(a || b)
            );
        }
        assert_eq!(
            Condition::Not(Box::new(boolean(a))).evaluate(&state),
            Ok(!a)
        );
    }
    let bad = comparison(Eq, Value::Int(1), Value::Float(1.0));
    for tree in [
        Condition::And(vec![boolean(false), bad.clone()]),
        Condition::Or(vec![boolean(true), bad]),
    ] {
        assert!(matches!(
            tree.evaluate(&state),
            Err(ConditionError::Operands { .. })
        ));
    }
    for tree in [Condition::And(vec![]), Condition::Or(vec![])] {
        assert_eq!(tree.evaluate(&state), Err(ConditionError::Empty));
    }
}

#[test]
fn state_reads_are_live_and_never_mutate_state() {
    let mut state = State::new();
    state.set("count", Value::Int(1)).unwrap();
    state.set("target", Value::Int(2)).unwrap();
    let tree = Condition::Compare {
        op: Lt,
        left: Operand::State("count".into()),
        right: Operand::State("target".into()),
    };
    let before = state.clone();
    assert_eq!(tree.evaluate(&state), Ok(true));
    assert_eq!(state, before);
    state.set("count", Value::Int(3)).unwrap();
    assert_eq!(tree.evaluate(&state), Ok(false));
    let literal = Condition::Compare {
        op: Eq,
        left: Operand::State("count".into()),
        right: Operand::Literal(Value::Int(3)),
    };
    assert_eq!(literal.evaluate(&state), Ok(true));
    let wrong_type = Condition::Compare {
        op: Eq,
        left: Operand::State("count".into()),
        right: Operand::Literal(Value::Float(3.0)),
    };
    assert!(matches!(
        Condition::Or(vec![boolean(true), wrong_type.clone()]).evaluate(&state),
        Err(ConditionError::Operands { .. })
    ));
    state.remove("count").unwrap();
    assert!(matches!(
        wrong_type.validate(&state),
        Err(ConditionError::Operands { .. })
    ));
    assert_eq!(tree.validate(&state), Ok(()));
    assert_eq!(
        tree.evaluate(&state),
        Err(ConditionError::State(StateError::MissingKey(
            "count".into()
        )))
    );
    // Removed declarations still type-check, but an evaluated read must not silently use a default.
    assert_eq!(
        Condition::And(vec![boolean(false), tree.clone()]).evaluate(&state),
        Ok(false)
    );
    assert_eq!(
        Condition::Or(vec![boolean(true), tree]).evaluate(&state),
        Ok(true)
    );
    let unknown = Condition::Compare {
        op: Eq,
        left: Operand::State("unknown".into()),
        right: Operand::Literal(Value::Int(0)),
    };
    assert_eq!(
        Condition::Or(vec![boolean(true), unknown]).evaluate(&state),
        Err(ConditionError::State(StateError::MissingKey(
            "unknown".into()
        )))
    );
}

#[test]
fn nonfinite_literals_are_rejected_even_in_skipped_branches() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for (left, right) in [(value, 0.0), (0.0, value)] {
            let bad = comparison(Eq, Value::Float(left), Value::Float(right));
            assert_eq!(
                Condition::Or(vec![boolean(true), bad]).evaluate(&State::new()),
                Err(ConditionError::State(StateError::NonFinite))
            );
        }
    }
}

#[test]
fn bounds_include_the_root_and_all_skipped_children() {
    let state = State::new();
    let mut tree = boolean(true);
    for _ in 1..MAX_DEPTH {
        tree = Condition::Not(Box::new(tree));
    }
    assert!(tree.evaluate(&state).is_ok());
    assert_eq!(
        Condition::Or(vec![boolean(true), tree.clone()]).evaluate(&state),
        Err(ConditionError::Depth)
    );
    assert_eq!(
        Condition::Not(Box::new(tree)).evaluate(&state),
        Err(ConditionError::Depth)
    );
    assert_eq!(
        Condition::Or(vec![boolean(true); MAX_NODES - 1]).evaluate(&state),
        Ok(true)
    );
    assert_eq!(
        Condition::Or(vec![boolean(true); MAX_NODES]).evaluate(&state),
        Err(ConditionError::Nodes)
    );
    let children = (0..16)
        .map(|_| Condition::And(vec![boolean(true); 15]))
        .collect();
    assert_eq!(
        Condition::Or(children).evaluate(&state),
        Err(ConditionError::Nodes)
    );
}

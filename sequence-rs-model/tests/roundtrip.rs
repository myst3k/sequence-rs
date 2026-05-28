//! Roundtrip tests over the spec's example payloads — proof that the
//! discriminated unions deserialize and re-serialize to the same wire shape.

use sequence_rs_model::account::AccountType;
use sequence_rs_model::rule::*;
use sequence_rs_model::transfer::*;

const RULE_JSON: &str = r#"{
  "id": "551ff9b6-ddf1-4110-b611-1b11044b72d4",
  "name": "Auto-save on deposit",
  "description": "Saves 20% of every incoming deposit",
  "status": "ENABLED",
  "trigger": { "type": "ON_FUNDS_TRANSFERRED", "accountId": "c7a7f26f-2ca5-4ae5-825a-70260591247c" },
  "steps": [
    {
      "conditions": { "condition": { "fact": "BALANCE", "operator": "GREATER_THAN", "value": 50000, "valueFact": null, "params": null } },
      "actions": [
        { "type": "PERCENTAGE", "percentageValue": 20.0, "percentageTarget": "INCOMING_AMOUNT",
          "source": {"id":"c7a7f26f-2ca5-4ae5-825a-70260591247c","type":"INCOME_SOURCE","name":null},
          "destination": {"id":"57ee255e-b1d7-4da4-8080-edbf783b0898","type":"POD","name":null},
          "groupIndex": 0, "upToEnabled": false, "isDirectDeposit": false, "limit": null, "achDescription": null }
      ]
    },
    {
      "conditions": null,
      "actions": [
        { "type": "FIXED", "amountInCents": 5000,
          "source": {"id":"c7a7f26f-2ca5-4ae5-825a-70260591247c","type":"INCOME_SOURCE","name":null},
          "destination": {"id":"fae66a7b-e93b-4d24-9ee3-8f07b8970e8e","type":"POD","name":null},
          "groupIndex": 0, "upToEnabled": true, "isDirectDeposit": false, "limit": null, "achDescription": null }
      ]
    }
  ],
  "createdAt": "2024-03-01T10:00:00Z",
  "updatedAt": "2024-03-15T14:30:00Z",
  "deletedAt": null
}"#;

#[test]
fn rule_deserializes_all_unions() {
    let rule: Rule = serde_json::from_str(RULE_JSON).expect("Rule should deserialize");

    match &rule.trigger {
        Trigger::OnFundsTransferred { account_id } => {
            assert_eq!(account_id, "c7a7f26f-2ca5-4ae5-825a-70260591247c");
        }
        other => panic!("unexpected trigger: {other:?}"),
    }

    assert_eq!(rule.steps.len(), 2);

    // Step 0: a leaf condition + a PERCENTAGE action.
    match rule.steps[0]
        .conditions
        .as_ref()
        .expect("step 0 has conditions")
    {
        ChainableRuleCondition::Condition { condition } => {
            assert!(matches!(condition.fact, RuleConditionFact::Balance));
            assert!(matches!(
                condition.operator,
                RuleConditionOperator::GreaterThan
            ));
            assert_eq!(condition.value, Some(50000.0));
            assert!(condition.value_fact.is_none());
            assert!(condition.params.is_none());
        }
        other => panic!("unexpected condition tree: {other:?}"),
    }
    let action0 = &rule.steps[0].actions[0];
    assert_eq!(action0.base.group_index, 0);
    assert!(!action0.base.up_to_enabled);
    assert!(matches!(
        action0.base.source.account_type,
        AccountType::IncomeSource
    ));
    match &action0.kind {
        RuleActionKind::Percentage {
            percentage_value,
            percentage_target,
        } => {
            assert_eq!(*percentage_value, 20.0);
            assert!(matches!(
                percentage_target,
                PercentageTarget::IncomingAmount
            ));
        }
        other => panic!("unexpected action kind: {other:?}"),
    }

    // Step 1: catch-all (no conditions) + a FIXED action.
    assert!(rule.steps[1].conditions.is_none());
    let action1 = &rule.steps[1].actions[0];
    assert!(action1.base.up_to_enabled);
    match &action1.kind {
        RuleActionKind::Fixed { amount_in_cents } => assert_eq!(*amount_in_cents, 5000),
        other => panic!("unexpected action kind: {other:?}"),
    }
}

#[test]
fn rule_action_reserializes_flat() {
    // The decisive check: `base` + `kind` + the `type` tag must serialize back
    // into a single flat object, not nested sub-objects.
    let rule: Rule = serde_json::from_str(RULE_JSON).unwrap();
    let value = serde_json::to_value(&rule).unwrap();
    let action = &value["steps"][1]["actions"][0];

    assert_eq!(action["type"], "FIXED");
    assert_eq!(action["amountInCents"], 5000); // from `kind`
    assert_eq!(action["groupIndex"], 0); // from `base`
    assert!(action["source"].is_object()); // from `base`
    assert_eq!(action["upToEnabled"], true);
    // No leaked wrapper keys from the two flattened fields.
    assert!(action.get("base").is_none());
    assert!(action.get("kind").is_none());
}

#[test]
fn rule_execution_trigger_details_variants() {
    let executed = r#"{
      "id": "4306b3e8-6e77-4c08-ab0b-bb33654af44c",
      "ruleId": "551ff9b6-ddf1-4110-b611-1b11044b72d4",
      "status": "EXECUTED",
      "createdAt": "2024-04-23T09:15:00Z",
      "triggerDetails": { "type": "ON_FUNDS_TRANSFERRED", "amountInCents": 250000 },
      "stepIndexMatched": 0,
      "conditionsNotMet": false,
      "transfersAttempted": 2, "transfersCompleted": 2, "transfersFailed": 0, "transfersPending": 0,
      "transferIds": ["809e5e0b-bb0b-49b2-867a-8b44d04d9179", "32a4182a-38b5-4058-98da-4d1b3d13ab72"],
      "errorMessage": null, "nextAttemptAt": null
    }"#;
    let exec: RuleExecution = serde_json::from_str(executed).unwrap();
    assert_eq!(exec.summary.rule_id, "551ff9b6-ddf1-4110-b611-1b11044b72d4");
    assert!(matches!(exec.summary.status, RuleExecutionStatus::Executed));
    assert_eq!(exec.step_index_matched, Some(0));
    assert_eq!(exec.transfer_ids.len(), 2);
    match exec.trigger_details {
        TriggerDetails::OnFundsTransferred { amount_in_cents } => {
            assert_eq!(amount_in_cents, Some(250000))
        }
        other => panic!("unexpected trigger details: {other:?}"),
    }

    let scheduled = r#"{
      "id": "4fde08bb-8f17-45ec-9d3f-a30c6ffc1351",
      "ruleId": "551ff9b6-ddf1-4110-b611-1b11044b72d4",
      "status": "EXECUTED",
      "createdAt": "2024-04-20T08:00:00Z",
      "triggerDetails": { "type": "SCHEDULED", "scheduledTime": "2024-04-20T08:00:00Z" },
      "stepIndexMatched": null,
      "conditionsNotMet": true,
      "transfersAttempted": 0, "transfersCompleted": 0, "transfersFailed": 0, "transfersPending": 0,
      "transferIds": [], "errorMessage": null, "nextAttemptAt": null
    }"#;
    let exec: RuleExecution = serde_json::from_str(scheduled).unwrap();
    assert!(exec.conditions_not_met);
    assert!(exec.step_index_matched.is_none());
    assert!(matches!(
        exec.trigger_details,
        TriggerDetails::Scheduled { .. }
    ));
}

#[test]
fn transfer_with_account_refs() {
    let json = r#"{
      "id": "809e5e0b-bb0b-49b2-867a-8b44d04d9179",
      "amountInCents": 100000,
      "direction": "INTERNAL",
      "origin": "RULE",
      "status": "COMPLETE",
      "source": { "id": "c7a7f26f-2ca5-4ae5-825a-70260591247c", "name": "Main Payroll", "type": "INCOME_SOURCE", "isDeleted": false },
      "destination": { "id": "c2cb3499-2491-4185-a6f5-1a3d281b875a", "name": "Emergency Fund", "type": "POD", "isDeleted": false },
      "ruleId": "551ff9b6-ddf1-4110-b611-1b11044b72d4",
      "ruleExecutionId": "4306b3e8-6e77-4c08-ab0b-bb33654af44c",
      "errorCode": null,
      "createdAt": "2024-04-23T09:15:00Z",
      "completedAt": "2024-04-23T09:15:04Z"
    }"#;
    let t: Transfer = serde_json::from_str(json).unwrap();
    assert!(matches!(t.direction, TransferDirection::Internal));
    assert!(matches!(t.origin, TransferOrigin::Rule));
    let src = t.source.expect("source present");
    assert_eq!(src.name, "Main Payroll");
    assert!(matches!(
        src.participant_type,
        TransferParticipantType::IncomeSource
    ));
    assert_eq!(src.is_deleted, Some(false));
    assert!(t.destination.is_some());
}

fn transfer_req(amount: i64, description: Option<&str>) -> CreateTransferRequest {
    CreateTransferRequest {
        source_account_id: "a".into(),
        destination_account_id: "b".into(),
        amount_in_cents: amount,
        description: description.map(str::to_string),
    }
}

#[test]
fn create_transfer_validation() {
    use sequence_rs_model::error::ModelError;

    // Valid: at/above the $1.00 floor, description within limits.
    assert!(transfer_req(100, None).validate().is_ok());
    assert!(transfer_req(5000, Some("Rent May")).validate().is_ok());
    assert!(transfer_req(100, Some("")).validate().is_ok());

    // Below the minimum.
    assert!(matches!(
        transfer_req(99, None).validate(),
        Err(ModelError::Validation(_))
    ));

    // Description too long (>10 chars).
    assert!(matches!(
        transfer_req(5000, Some("12345678901")).validate(),
        Err(ModelError::Validation(_))
    ));

    // Description with a disallowed character.
    assert!(matches!(
        transfer_req(5000, Some("Rent-May")).validate(),
        Err(ModelError::Validation(_))
    ));
}

/// Build a flat action object: the seven shared `base` fields merged with the
/// variant-specific `extra` fields.
fn action_json(extra: serde_json::Value) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "source": { "id": "s", "type": "POD", "name": null },
        "destination": { "id": "d", "type": "POD", "name": null },
        "groupIndex": 1,
        "upToEnabled": false,
        "isDirectDeposit": true,
        "limit": { "period": "PER_MONTH", "amountInCents": 100000 },
        "achDescription": "rent"
    });
    let map = obj.as_object_mut().unwrap();
    for (k, v) in extra.as_object().unwrap() {
        map.insert(k.clone(), v.clone());
    }
    obj
}

fn parse_action(extra: serde_json::Value) -> RuleAction {
    serde_json::from_value(action_json(extra)).expect("action should deserialize")
}

/// Every `RuleAction` variant deserializes, its shared `base` is populated, and
/// it re-serializes to a flat object carrying the `type` tag.
#[test]
fn all_rule_action_variants_roundtrip() {
    use serde_json::json;

    let cases: Vec<(serde_json::Value, &str)> = vec![
        (json!({ "type": "FIXED", "amountInCents": 5000 }), "FIXED"),
        (
            json!({ "type": "PERCENTAGE", "percentageValue": 20.0, "percentageTarget": "SOURCE_ACCOUNT" }),
            "PERCENTAGE",
        ),
        (
            json!({
                "type": "TOP_UP",
                "amountInCents": null,
                "nextPaymentMinimumAccount": { "id": "n", "type": "EXTERNAL_ACCOUNT", "name": "Card" },
                "currentBalanceAccount": null,
                "lastStatementBalanceAccount": null
            }),
            "TOP_UP",
        ),
        (
            json!({ "type": "ROUND_DOWN", "amountInCents": 100 }),
            "ROUND_DOWN",
        ),
        (
            json!({ "type": "NEXT_PAYMENT_MINIMUM" }),
            "NEXT_PAYMENT_MINIMUM",
        ),
        (json!({ "type": "TOTAL_AMOUNT_DUE" }), "TOTAL_AMOUNT_DUE"),
        (
            json!({ "type": "LAST_STATEMENT_BALANCE" }),
            "LAST_STATEMENT_BALANCE",
        ),
        (
            json!({ "type": "PERCENTAGE_LIABILITY_BALANCE", "percentageValue": 50.0 }),
            "PERCENTAGE_LIABILITY_BALANCE",
        ),
    ];

    for (extra, expected_tag) in cases {
        let action = parse_action(extra);
        // Shared base is always populated, regardless of variant.
        assert_eq!(
            action.base.group_index, 1,
            "{expected_tag}: base.group_index"
        );
        assert!(
            action.base.is_direct_deposit,
            "{expected_tag}: base.is_direct_deposit"
        );
        assert!(action.base.limit.is_some(), "{expected_tag}: base.limit");
        assert_eq!(action.base.ach_description.as_deref(), Some("rent"));

        // Re-serializes flat, with the tag and base fields side by side.
        let v = serde_json::to_value(&action).unwrap();
        assert_eq!(v["type"], expected_tag, "{expected_tag}: tag");
        assert!(v["source"].is_object(), "{expected_tag}: flat base.source");
        assert!(v.get("base").is_none() && v.get("kind").is_none());
    }
}

#[test]
fn top_up_target_fields_parse() {
    let action = parse_action(serde_json::json!({
        "type": "TOP_UP",
        "amountInCents": null,
        "nextPaymentMinimumAccount": { "id": "n", "type": "EXTERNAL_ACCOUNT", "name": "Card" },
        "currentBalanceAccount": null,
        "lastStatementBalanceAccount": null
    }));
    match action.kind {
        RuleActionKind::TopUp {
            amount_in_cents,
            next_payment_minimum_account,
            current_balance_account,
            last_statement_balance_account,
        } => {
            assert!(amount_in_cents.is_none());
            assert_eq!(
                next_payment_minimum_account.unwrap().name.as_deref(),
                Some("Card")
            );
            assert!(current_balance_account.is_none());
            assert!(last_statement_balance_account.is_none());
        }
        other => panic!("expected TopUp, got {other:?}"),
    }
}

/// A JSON integer where the model expects a float must still parse (numbers go
/// through serde's value coercion).
#[test]
fn percentage_accepts_integer_valued_float() {
    let action = parse_action(serde_json::json!({
        "type": "PERCENTAGE",
        "percentageValue": 20,
        "percentageTarget": "INCOMING_AMOUNT"
    }));
    match action.kind {
        RuleActionKind::Percentage {
            percentage_value, ..
        } => assert_eq!(percentage_value, 20.0),
        other => panic!("expected Percentage, got {other:?}"),
    }
}

/// Nested `all`/`any`/`condition` trees deserialize recursively.
#[test]
fn nested_chainable_conditions() {
    let json = serde_json::json!({
        "all": [
            { "condition": { "fact": "DATE", "operator": "GREATER_THAN", "value": 15, "valueFact": null, "params": null } },
            { "any": [
                { "condition": { "fact": "BALANCE", "operator": "LESS_THAN", "value": null, "valueFact": "TRANSFER_AMOUNT", "params": { "accountId": "acc-1" } } }
            ] }
        ]
    });
    let cond: ChainableRuleCondition = serde_json::from_value(json).unwrap();
    let ChainableRuleCondition::All { all } = cond else {
        panic!("expected All");
    };
    assert_eq!(all.len(), 2);
    assert!(matches!(all[0], ChainableRuleCondition::Condition { .. }));
    let ChainableRuleCondition::Any { any } = &all[1] else {
        panic!("expected nested Any");
    };
    assert_eq!(any.len(), 1);
    match &any[0] {
        ChainableRuleCondition::Condition { condition } => {
            assert!(matches!(condition.fact, RuleConditionFact::Balance));
            assert_eq!(
                condition.value_fact,
                Some(RuleConditionValueFact::TransferAmount)
            );
            assert_eq!(
                condition.params.as_ref().unwrap().account_id.as_deref(),
                Some("acc-1")
            );
        }
        other => panic!("expected leaf condition, got {other:?}"),
    }
}

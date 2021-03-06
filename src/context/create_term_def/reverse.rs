//! Part of create term definition algorithm, for reverse term definitions.

use std::{collections::HashMap, convert::TryFrom};

use anyhow::anyhow;
use serde_json::{Map as JsonMap, Value};

use crate::{
    context::{
        definition::{Container, ContainerItem, DefinitionBuilder},
        Context, ValueWithBase,
    },
    error::{ErrorCode, Result},
    expand::iri::ExpandIriOptions,
    iri::is_absolute_ref_or_blank_node_ident,
    json::Nullable,
    processor::Processor,
    remote::LoadRemoteDocument,
    syntax::has_form_of_keyword,
};

/// Runs rest of the create term definition algorithm for the case `@reverse` exists.
///
/// See <https://www.w3.org/TR/2019/WD-json-ld11-api-20191112/#create-term-definition>
// Step 14
#[allow(clippy::too_many_arguments)] // TODO: FIXME
pub(crate) async fn run_for_reverse<L: LoadRemoteDocument>(
    processor: &Processor<L>,
    active_context: &mut Context,
    local_context: ValueWithBase<'_, &JsonMap<String, Value>>,
    term: &str,
    defined: &mut HashMap<String, bool>,
    value: &JsonMap<String, Value>,
    reverse: &Value,
    mut definition: DefinitionBuilder,
) -> Result<()> {
    // Step 14.1
    if value.contains_key("@id") || value.contains_key("@nest") {
        return Err(
            ErrorCode::InvalidReverseProperty.and_source(anyhow!("Found `@id` or `@nest` entries"))
        );
    }
    // Step 14.2
    let reverse = match reverse {
        Value::String(s) => s,
        v => {
            return Err(ErrorCode::InvalidIriMapping
                .and_source(anyhow!("Expected string as @reverse but got {:?}", v)))
        }
    };
    // Step 14.3
    if has_form_of_keyword(reverse) {
        // FIXME: Generate a warning.
        return Ok(());
    }
    // Step 14.4
    let reverse = ExpandIriOptions::mutable(active_context, local_context, defined)
        .vocab(true)
        .expand_str(processor, reverse)
        .await?
        .ok_or_else(|| {
            ErrorCode::InvalidIriMapping
                .and_source(anyhow!("@reverse ({:?}) is expanded to `null`", reverse))
        })?;
    if is_absolute_ref_or_blank_node_ident(&reverse) {
        definition.set_iri(reverse);
    } else {
        return Err(ErrorCode::InvalidIriMapping.and_source(anyhow!(
            "Expanded @reverse value ({:?}) is neither an IRI nor blank node identifier",
            reverse
        )));
    }
    // Step 14.5
    process_conatiner(value, &mut definition)?;
    // Step 14.6
    definition.set_reverse(true);
    // Step 14.7
    let definition = definition.build();
    active_context
        .term_definitions
        .insert(term.to_owned(), Nullable::Value(definition));
    *defined
        .get_mut(term)
        .expect("Should never fail: inserted before") = true;

    Ok(())
}

/// Processes the container mapping if available.
fn process_conatiner(
    value: &JsonMap<String, Value>,
    definition: &mut DefinitionBuilder,
) -> Result<()> {
    // Step 14.5
    if let Some(container) = value.get("@container") {
        let container = Nullable::<Container>::try_from(container)
            .map_err(|e| ErrorCode::InvalidContainerMapping.and_source(e))?;
        // > If _value_ contains an `@container` entry, set the container mapping of _definition_
        // > to an array containing its value; if its value is neither `@set`, nor `@index`, nor
        // > `null`, an `invalid reverse property` error has been detected (reverse properties only
        // > support set- and index-containers) and processing is aborted.
        match container.map(|c| c.get_single_item()) {
            Nullable::Null
            | Nullable::Value(Some(ContainerItem::Set))
            | Nullable::Value(Some(ContainerItem::Index)) => {
                definition.set_container(container);
                Ok(())
            }
            _ => Err(ErrorCode::InvalidReverseProperty
                .and_source(anyhow!("`@container` = {:?}", container))),
        }
    } else {
        Ok(())
    }
}

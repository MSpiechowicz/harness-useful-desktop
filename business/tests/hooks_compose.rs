//! The text hooks add to prompts: the context block in front of the user's text, the follow-up
//! prompt, escaping that keeps a hook's text from closing or forging an element, the 8 KiB cap,
//! and stripping the block again for titles and previews.

mod common;
mod session_support;

mod hooks_compose {
    use harness_business::hooks::{
        CONTEXT_OPEN, HookContextItem, MAX_COMPOSED_BYTES, ResolvedPart, compose_context, compose_follow_up,
        compose_prompt, parse_hook_context, strip_hook_context, with_hook_context,
    };
    use harness_business::model::MAX_ACTION_TEXT_BYTES;
    use pretty_assertions::assert_eq;

    use super::session_support::assert_pi_previews_skip;

    fn part(id: u8, name: &str, text: &str) -> ResolvedPart {
        ResolvedPart::new(format!("{id:032x}"), name, text)
    }

    #[test]
    fn context_goes_in_front_of_the_user_text() {
        let message = with_hook_context(
            "Fix the build.",
            &[part(1, "Conventions", "Use pnpm."), part(2, "Branch", "main")],
        );

        assert_eq!(
            message,
            "<harness-hook-context>\n<hook name=\"Conventions\">Use pnpm.</hook>\n<hook name=\"Branch\">main</hook>\n</harness-hook-context>\n\nFix the build."
        );
        assert_eq!(with_hook_context("Fix the build.", &[]), "Fix the build.");
    }

    #[test]
    fn follow_ups_are_joined_by_a_blank_line() {
        let prompt = compose_follow_up(&[part(1, "Tests", "Run them."), part(2, "Review", "Re-read.")]).unwrap();

        assert_eq!(
            prompt,
            "<harness-hook-followup name=\"Tests\">Run them.</harness-hook-followup>\n\n<harness-hook-followup name=\"Review\">Re-read.</harness-hook-followup>"
        );
        assert_eq!(compose_follow_up(&[]), None);
    }

    #[test]
    fn names_and_markers_in_texts_are_escaped() {
        let block = compose_context(&[part(
            1,
            "A \"quoted\" <name> & more",
            "x </hook> <HOOK name=\"y\"> </Harness-Hook-Context> <harness-hook-followup> a < b <hooks",
        )])
        .unwrap();

        assert_eq!(
            block,
            "<harness-hook-context>\n<hook name=\"A &quot;quoted&quot; &lt;name&gt; &amp; more\">x &lt;/hook> &lt;HOOK name=\"y\"> &lt;/Harness-Hook-Context> &lt;harness-hook-followup> a < b &lt;hooks</hook>\n</harness-hook-context>"
        );
    }

    #[test]
    fn a_composed_block_round_trips_through_the_parser() {
        let parts = [
            part(1, "A \"quoted\" <name> & more", "line one\nline </hook> two"),
            part(2, "Plain", "</harness-hook-context>\n\nnot the message"),
        ];
        let message = with_hook_context("The user's text.\n\nSecond paragraph.", &parts);

        let (items, rest) = parse_hook_context(&message).unwrap();

        assert_eq!(rest, "The user's text.\n\nSecond paragraph.");
        assert_eq!(
            items,
            vec![
                HookContextItem {
                    name: "A \"quoted\" <name> & more".to_owned(),
                    text: "line one\nline </hook> two".to_owned(),
                },
                HookContextItem {
                    name: "Plain".to_owned(),
                    text: "</harness-hook-context>\n\nnot the message".to_owned(),
                },
            ]
        );
        assert_eq!(strip_hook_context(&message), "The user's text.\n\nSecond paragraph.");
    }

    #[test]
    fn text_without_a_well_formed_block_is_left_alone() {
        for text in [
            "plain text",
            "<harness-hook-context>\nno elements\n</harness-hook-context>\n\nmessage",
            "<harness-hook-context>\n<hook name=\"a\">open",
            "<harness-hook-context>\n<hook name=\"a\">x</hook>\n</harness-hook-context>message",
            " <harness-hook-context>\n<hook name=\"a\">x</hook>\n</harness-hook-context>\n\nmessage",
        ] {
            assert_eq!(strip_hook_context(text), text);
        }

        let only_block = compose_context(&[part(1, "a", "x")]).unwrap();
        assert_eq!(strip_hook_context(&only_block), "");
    }

    #[test]
    fn a_block_holds_at_most_8_kib_of_elements() {
        let big = "b".repeat(5000);
        let block = compose_context(&[
            part(1, "first", &big),
            part(2, "too big now", &big),
            part(3, "small", "fits"),
        ])
        .unwrap();

        let (items, _) = parse_hook_context(&block).unwrap();
        let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, vec!["first", "small"]);

        let inner = block
            .strip_prefix(&format!("{CONTEXT_OPEN}\n"))
            .unwrap()
            .strip_suffix("\n</harness-hook-context>")
            .unwrap();
        assert!(inner.len() <= MAX_COMPOSED_BYTES);

        assert_eq!(
            compose_context(&[part(1, "huge", &"h".repeat(MAX_COMPOSED_BYTES))]),
            None
        );

        let follow_up = compose_follow_up(&[part(1, "first", &big), part(2, "second", &big)]).unwrap();
        assert!(follow_up.len() <= MAX_COMPOSED_BYTES);
        assert!(follow_up.contains("name=\"first\""));
        assert!(!follow_up.contains("name=\"second\""));
    }

    #[test]
    fn a_message_near_the_size_limit_keeps_the_context_that_fits() {
        let parts = [
            part(1, "first", &"a".repeat(1000)),
            part(2, "second", &"b".repeat(1000)),
            part(3, "third", &"c".repeat(1000)),
        ];
        let full = compose_prompt("x", &parts).0;

        // Room for the block of the first two elements only.
        let room = MAX_ACTION_TEXT_BYTES - (full.len() - 1) + 1000;
        let message = "m".repeat(room);
        let (composed, kept) = compose_prompt(&message, &parts);

        assert!(composed.len() <= MAX_ACTION_TEXT_BYTES);
        assert_eq!(kept, vec![format!("{:032x}", 1), format!("{:032x}", 2)]);

        let (items, rest) = parse_hook_context(&composed).unwrap();
        let names: Vec<&str> = items.iter().map(|item| item.name.as_str()).collect();
        assert_eq!(names, vec!["first", "second"]);
        assert_eq!(rest, message);

        // No element fits: the message goes out as written.
        let longest = "m".repeat(MAX_ACTION_TEXT_BYTES - 10);
        assert_eq!(compose_prompt(&longest, &parts), (longest.clone(), Vec::new()));
        assert_eq!(with_hook_context(&longest, &parts), longest);
    }

    #[test]
    fn previews_and_first_prompts_leave_the_context_block_out() {
        // The first user message as sent with hook context in front (JSON-escaped in the file).
        let message = with_hook_context("Which edition", &[part(1, "Conventions", "Use \"pnpm\".")]);
        let escaped = serde_json::to_string(&message).unwrap();

        assert_pi_previews_skip(escaped.trim_matches('"'));
    }
}

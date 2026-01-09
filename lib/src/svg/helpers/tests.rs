use super::*;

#[test]
fn scope_helper_basics() {
    let template = "{{#scope test_var=1}}Test var is: {{@test_var}}{{/scope}}";
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("scope", Box::new(ScopeHelper));
    let data = serde_json::json!({ "test": 3 });
    let rendered = handlebars.render_template(template, &data).unwrap();
    assert_eq!(rendered, "Test var is: 1");
}

#[test]
fn reassigning_scope_vars() {
    let template = r#"
            {{#scope test_var="test"}}
                {{#set "test_var"}}"{{@test_var}} value"{{/set}}
                Test var is: {{@test_var}}
            {{/scope}}
        "#;

    let mut handlebars = Handlebars::new();
    handlebars.register_helper("scope", Box::new(ScopeHelper));
    handlebars.register_helper("set", Box::new(SetHelper));
    let data = serde_json::json!({ "test": 3 });
    let rendered = handlebars.render_template(template, &data).unwrap();
    assert_eq!(rendered.trim(), "Test var is: test value");
}

#[test]
fn reassigning_scope_vars_via_appending() {
    let template = r#"
            {{#scope test_var="test"}}
                {{#set "test_var" append=true}} value{{/set}}
                {{#set "test_var" append=true}}!{{/set}}
                Test var is: {{@test_var}}
            {{/scope}}
        "#;

    let mut handlebars = Handlebars::new();
    handlebars.register_helper("scope", Box::new(ScopeHelper));
    handlebars.register_helper("set", Box::new(SetHelper));
    let data = serde_json::json!({ "test": 3 });
    let rendered = handlebars.render_template(template, &data).unwrap();
    assert_eq!(rendered.trim(), "Test var is: test value!");
}

#[test]
fn scope_helper_with_control_flow() {
    let template = r#"
            {{#scope result=""}}
                {{#each values}}
                    {{#if @first}}
                        {{set result=this}}
                    {{else}}
                        {{#set "result"}}"{{@../result}}, {{this}}"{{/set}}
                    {{/if}}
                {{/each}}
                Concatenated: {{@result}}
            {{/scope}}
        "#;

    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("scope", Box::new(ScopeHelper));
    handlebars.register_helper("set", Box::new(SetHelper));
    let data = serde_json::json!({ "values": ["foo", "bar", "baz"] });
    let rendered = handlebars.render_template(template, &data).unwrap();
    assert_eq!(rendered.trim(), "Concatenated: foo, bar, baz");
}

#[test]
fn add_helper_basics() {
    let template = "{{add 1 2 5}}";
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("add", Box::new(OpsHelper::Add));
    let rendered = handlebars.render_template(template, &()).unwrap();
    assert_eq!(rendered, "8");
}

#[test]
fn min_helper_basics() {
    let template = "{{min 2 -1 5}}";
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("min", Box::new(OpsHelper::Min));
    let rendered = handlebars.render_template(template, &()).unwrap();
    assert_eq!(rendered, "-1");
}

#[test]
fn add_with_scope_var() {
    let template = "
            {{#scope lines=0 margins=0}}
                {{#each values}}
                    {{set lines=(add @../lines input.line_count output.line_count)}}
                    {{#if (eq output.line_count 0) }}
                        {{set margins=(add @../margins 1)}}
                    {{else}}
                        {{set margins=(add @../margins 2)}}
                    {{/if}}
                {{/each}}
                {{@lines}}, {{@margins}}
            {{/scope}}
        ";

    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("scope", Box::new(ScopeHelper));
    handlebars.register_helper("set", Box::new(SetHelper));
    handlebars.register_helper("add", Box::new(OpsHelper::Add));

    let data = serde_json::json!({
        "values": [{
            "input": { "line_count": 1 },
            "output": { "line_count": 2 },
        }, {
            "input": { "line_count": 2 },
            "output": { "line_count": 0 },
        }]
    });
    let rendered = handlebars.render_template(template, &data).unwrap();
    assert_eq!(rendered.trim(), "5, 3");
}

#[test]
fn rounding_helper() {
    let template = "
            {{round 10.5}}, {{round 10.5 digits=2}}, {{round (mul 14 (div 1050 1000)) digits=2}}
        ";
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("mul", Box::new(OpsHelper::Mul));
    handlebars.register_helper("div", Box::new(OpsHelper::Div));
    handlebars.register_helper("round", Box::new(RoundHelper));
    let rendered = handlebars.render_template(template, &()).unwrap();
    assert_eq!(rendered.trim(), "11, 10.5, 14.7");
}

#[test]
fn rounding_helper_with_mode() {
    let template = r#"
            {{round 10.6 mode="nearest"}}, {{round 10.513 digits=2 mode="down"}}, {{round 11.001 digits=1 mode="up"}}
        "#;
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("mul", Box::new(OpsHelper::Mul));
    handlebars.register_helper("div", Box::new(OpsHelper::Div));
    handlebars.register_helper("round", Box::new(RoundHelper));
    let rendered = handlebars.render_template(template, &()).unwrap();
    assert_eq!(rendered.trim(), "11, 10.51, 11.1");
}

#[test]
fn line_counter() {
    let template = "{{count_lines text}}";
    let text = "test\ntest test";

    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("count_lines", Box::new(LineCounter));
    let data = serde_json::json!({ "text": text });
    let rendered = handlebars.render_template(template, &data).unwrap();
    assert_eq!(rendered.trim(), "2");
}

#[test]
fn line_splitter() {
    let template = "{{#each (split_lines text)}}{{this}}<br/>{{/each}}";
    let text = "test\nother test";

    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("split_lines", Box::new(LineSplitter));
    let data = serde_json::json!({ "text": text });
    let rendered = handlebars.render_template(template, &data).unwrap();
    assert_eq!(rendered.trim(), "test<br/>other test<br/>");

    let text = "test\nother test\n";
    let data = serde_json::json!({ "text": text });
    let rendered = handlebars.render_template(template, &data).unwrap();
    assert_eq!(rendered.trim(), "test<br/>other test<br/>");
}

#[test]
fn range_helper_with_each_block() {
    let template = "{{#each (range 0 4)}}{{@index}}: {{lookup ../xs @index}}, {{/each}}";

    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("range", Box::new(RangeHelper));
    let data = serde_json::json!({ "xs": [2, 3, 5, 8] });
    let rendered = handlebars.render_template(template, &data).unwrap();
    assert_eq!(rendered.trim(), "0: 2, 1: 3, 2: 5, 3: 8,");
}

#[test]
fn repeat_helper_basics() {
    let template = "{{repeat \"█\" 5}}";
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("repeat", Box::new(RepeatHelper));

    let rendered = handlebars.render_template(template, &()).unwrap();
    assert_eq!(rendered.trim(), "█████");
}

#[test]
fn set_helper() {
    let template =
        "{{#scope test_var=1}}{{set test_var=(add @test_var 1)}}Test var: {{@test_var}}{{/scope}}";
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("scope", Box::new(ScopeHelper));
    handlebars.register_helper("set", Box::new(SetHelper));
    handlebars.register_helper("add", Box::new(OpsHelper::Add));

    let rendered = handlebars.render_template(template, &()).unwrap();
    assert_eq!(rendered, "Test var: 2");
}

#[test]
fn set_helper_as_block() {
    let template = r#"{{#scope test_var=1 greet="Hello"~}}
            {{~#set "test_var"}}{{add @test_var 1}}{{/set~}}
            {{~#set "greet" append=true}}, world!{{/set~}}
            {{@greet}} {{@test_var}}
        {{~/scope}}"#;
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("scope", Box::new(ScopeHelper));
    handlebars.register_helper("set", Box::new(SetHelper));
    handlebars.register_helper("add", Box::new(OpsHelper::Add));

    let rendered = handlebars.render_template(template, &()).unwrap();
    assert_eq!(rendered, "Hello, world! 2");
}

#[test]
fn set_helper_with_scope() {
    let template = "
            {{~#scope test_var=1~}}
              {{~#each [1, 2, 3] as |num|~}}
                {{~set test_var=(add @../test_var num)}}-{{@../test_var~}}
              {{~/each~}}
            {{~/scope~}}
        ";
    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("scope", Box::new(ScopeHelper));
    handlebars.register_helper("set", Box::new(SetHelper));
    handlebars.register_helper("add", Box::new(OpsHelper::Add));

    let rendered = handlebars.render_template(template, &()).unwrap();
    assert_eq!(rendered.trim(), "-2-4-7");
}

#[test]
fn embedded_scopes() {
    let template = r"
            {{~#scope x=1 z=100~}}
                x={{@x}},
                {{~#scope x=2 y=3~}}
                  x={{@x}},y={{@y}},
                  {{~set x=4 y=5~}}
                  x={{@x}},y={{@y}},z={{@z}},
                  {{~set z=-100~}}
                  z={{@z}},
                {{~/scope~}}
                x={{@x}},z={{@z}}
            {{~/scope~}}
        ";

    let mut handlebars = Handlebars::new();
    handlebars.set_strict_mode(true);
    handlebars.register_helper("scope", Box::new(ScopeHelper));
    handlebars.register_helper("set", Box::new(SetHelper));

    let rendered = handlebars.render_template(template, &()).unwrap();
    assert_eq!(
        rendered.trim(),
        "x=1,x=2,y=3,x=4,y=5,z=100,z=-100,x=1,z=-100"
    );
}

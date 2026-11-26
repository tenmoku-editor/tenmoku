; Upstream: monaqa/tree-sitter-mermaid queries/highlights.scm at
; 90ae195b31933ceb9d079abfa8a3ad0a36fee4cc,
; https://github.com/monaqa/tree-sitter-mermaid, under the grammar's MIT
; licence (LICENSE). Its captures are Neovim's, so later patterns win.
; Changed: upstream's patterns are unchanged, and patterns for what they leave
; plain (labels and other free text, the actor and showData words, directions,
; class members and mind maps) follow them under "Additions".

[
 "sequenceDiagram"
 "classDiagram"
 "classDiagram-v2"
 "stateDiagram"
 "stateDiagram-v2"
 "gantt"
 "pie"
 "flowchart"
 "erdiagram"

 "participant"
 "as"
 "activate"
 "deactivate"
 "note "
 "over"
 "link"
 "links"
 ; "left of"
 ; "right of"
 "properties"
 "details"
 "title"
 "loop"
 "rect"
 "opt"
 "alt"
 "else"
 "par"
 "and"
 "end"
 (sequence_stmt_autonumber)
 (note_placement_left)
 (note_placement_right)

 "class"

 "state "

 "dateformat"
 "inclusiveenddates"
 "topaxis"
 "axisformat"
 "includes"
 "excludes"
 "todaymarker"
 "title"
 "section"

 "direction"
 "subgraph"

 ] @keyword

[
 (comment)
 ] @comment

[
 ":"
 (sequence_signal_plus_sign)
 (sequence_signal_minus_sign)

 (class_visibility_public)
 (class_visibility_private)
 (class_visibility_protected)
 (class_visibility_internal)

 (state_division)
 ] @punctuation.delimiter

[
 "("
 ")"
 "{"
 "}"
 ] @punctuation.bracket

[
 "-->"
 (solid_arrow)
 (dotted_arrow)
 (solid_open_arrow)
 (dotted_open_arrow)
 (solid_cross)
 (dotted_cross)
 (solid_point)
 (dotted_point)
 ] @operator

[
 (class_reltype_aggregation)
 (class_reltype_extension)
 (class_reltype_composition)
 (class_reltype_dependency)
 (class_linetype_solid)
 (class_linetype_dotted)
 "&"
 ] @operator

(sequence_actor) @field
(class_name) @field

(state_name) @field

(gantt_task_text) @field

[
 (class_annotation_line)
 (class_stmt_annotation)
 (class_generics)

 (state_annotation_fork)
 (state_annotation_join)
 (state_annotation_choice)
 ] @type

(directive) @include

(pie_label) @string
(pie_value) @float

[
(flowchart_direction_lr)
(flowchart_direction_rl)
(flowchart_direction_tb)
(flowchart_direction_bt)
 ] @constant

(flow_vertex_id) @field

[
 (flow_link_arrow)
 (flow_link_arrow_start)
 ] @operator

(flow_link_arrowtext "|" @punctuation.bracket)

(flow_vertex_square        [ "[" "]" ]   @punctuation.bracket )
(flow_vertex_circle        ["((" "))"]   @punctuation.bracket )
(flow_vertex_ellipse       ["(-" "-)"]   @punctuation.bracket )
(flow_vertex_stadium       ["([" "])"]   @punctuation.bracket )
(flow_vertex_subroutine    ["[[" "]]"]   @punctuation.bracket )
(flow_vertex_rect          ["[|" "|]"]   @punctuation.bracket )
(flow_vertex_cylinder      ["[(" ")]"]   @punctuation.bracket )
(flow_vertex_round         ["(" ")"]     @punctuation.bracket )
(flow_vertex_diamond       ["{" "}"]     @punctuation.bracket )
(flow_vertex_hexagon       ["{{" "}}"]   @punctuation.bracket )
(flow_vertex_odd           [">" "]"]     @punctuation.bracket )
(flow_vertex_trapezoid     ["[/" "\\]"]  @punctuation.bracket )
(flow_vertex_inv_trapezoid ["[\\" "/]"]  @punctuation.bracket )
(flow_vertex_leanright     ["[/" "/]"]   @punctuation.bracket )
(flow_vertex_leanleft      ["[\\" "\\]"] @punctuation.bracket )

(flow_stmt_subgraph ["[" "]"] @punctuation.bracket )

[
 (er_cardinarity_zero_or_one)
 (er_cardinarity_zero_or_more)
 (er_cardinarity_one_or_more)
 (er_cardinarity_only_one)
 (er_reltype_non_identifying)
 (er_reltype_identifying)
 ] @operator

(er_entity_name) @field

(er_attribute_type) @type
(er_attribute_name) @field

[
 (er_attribute_key_type_pk)
 (er_attribute_key_type_fk)
 ] @keyword

(er_attribute_comment) @string

; Additions

[
 "actor"
 (pie_showdata)
 (state_hide_empty_description)

 (direction_lr)
 (direction_rl)
 (direction_tb)
 (direction_bt)
 ] @keyword

; Labels and free text

[
 (flow_text_literal)
 (flow_text_quoted)
 (flow_arrow_text)
 (flow_vertex_text)

 (sequence_alias)
 (sequence_text)
 (sequence_stmt_title (title))

 (cardinality)
 (class_label)

 (state_description)
 (note_content)

 (gantt_title)
 (gantt_section)
 (gantt_date_format)
 (gantt_axis_format)
 (gantt_end_dates)
 (gantt_top_axis)
 (gantt_includes)
 (gantt_excludes)
 (gantt_today_marker)
 (gantt_task_data)

 (pie_title)

 (er_role)
 ] @string

(state_id) @field

; Class members

(class_method_line) @field

[
 (class_classifier_abstract)
 (class_classifier_static)
 ] @operator

(class_style_name) @constant

; Mind maps

(mmap_node_id) @field

[
 (mmap_text)
 (mmap_markdown)
 ] @string

(mmap_icon) @constant
(mmap_icon ")" @constant)

(mmap_class) @attribute

(mmap_node_square    ["[" "]"]   @punctuation.bracket)
(mmap_node_circle    ["((" "))"] @punctuation.bracket)
(mmap_node_cloud     [")" "("]   @punctuation.bracket)
(mmap_node_bang      ["))" "(("] @punctuation.bracket)
(mmap_node_hexagon   ["{{" "}}"] @punctuation.bracket)

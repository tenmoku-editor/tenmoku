#!/usr/bin/env wolframscript
(* A small package of vector helpers.
   Comments are blocks, and (* they nest (* to any depth *) *). *)

BeginPackage["Vectors`", {"GeneralUtilities`"}]

square::usage = "square[x, y] gives x^2 + y for an integer y."
scale::negative = "Expected a positive factor, not `1`.\n\tSee \"scale\"."

Begin["`Private`"]

(* Definitions, patterns and conditions *)
square[x_, y_Integer] := x^2 + y
scale[x_?NumericQ, factor_: 2] /; x > 0 := factor x
scale[x_, ___] := (Message[scale::negative, x]; $Failed)
total[first_, rest__, extra___] := Plus[first, rest, extra]
shift[x_, offset_.] := x + offset

SetAttributes[square, {Listable, Protected}]
Needs["Developer`"]

(* Numbers, constants and named characters *)
constants = {1.5*^10, 2^^101, 16^^ff.8, 3.14`20, 42, .5};
flags = {True, False, Null, None, All, Automatic, Infinity, Pi, E, I};
angle = \[Alpha] + 2 Pi - 30 Degree;
label = "Angle \[Alpha] is \"small\"\n";
arrow = \[LeftAngleBracket];
paths = {$Version, $InputFileName, Missing["NotAvailable"]};

(* Application, mapping and pure functions *)
squares = square[#, 1] & /@ Range[10];
sums = Plus @@ {1, 2, 3};
pairs = {#1, #2, ##} & @@@ {{1, 2}, {3, 4}};
named = #name & @ <|"name" -> "Ada"|>;
roots = squares // Sqrt // N
negated = Minus @ First @ squares;
joined = a ~Join~ b;
composed = (f @* g)[x];

(* Rules, parts and spans *)
rules = {x -> 1, y :> RandomReal[], z -> x + y};
result = x + y /. rules;
head = constants[[1]];
middle = constants[[2 ;; -2]];
every = constants[[1 ;; ;; 2]];
data = <|"a" -> 1, "b" -> {2, 3}|>;
data["a"] += 1;
data[["b", 1]] = 0

(* Scoping and control flow *)
describe[n_Integer] := Module[{total = 0, i},
  Do[total += i, {i, n}];
  If[total > 10 && EvenQ[total], Print["big"], Print["small"]];
  Which[total == 0, "zero", total != 1, "many", True, "one"]
]
area = Block[{r = 2}, With[{pi = N[Pi]}, pi r^2]];
table = Table[If[i === j, 1, 0], {i, 3}, {j, 3}];
previous = {%, %%, %3};
settings = <<"settings.wl"
extra = << Developer`
table >> "table.txt"
population = =[population of France]
spec = "Integer"::["Positive"]

End[]
EndPackage[]

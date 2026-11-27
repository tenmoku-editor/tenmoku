import Std.Data.HashMap

/-!
# A tour of Lean 4

Shapes, a small expression type, and a few proofs.
-/

/- A block comment, with a /- nested -/ one inside. -/

universe u

namespace Tour

open Nat

variable {α : Type u}

/-- A point in the plane. -/
structure Point where
  x : Nat
  y : Nat := 0
  deriving Repr, BEq

/-- Arithmetic expressions. -/
inductive Expr where
  | lit (n : Nat)
  | add (a b : Expr)
  | mul (a b : Expr)

def Expr.eval : Expr → Nat
  | .lit n => n
  | .add a b => a.eval + b.eval
  | .mul a b => a.eval * b.eval

-- Pattern matching on two arguments.
def zip {β : Type u} : List α → List β → List (α × β)
  | x :: xs, y :: ys => (x, y) :: zip xs ys
  | _, _ => []

def describe (n : Nat) (loud : Bool := false) : String :=
  match n, loud with
  | 0, _ => "zero"
  | 1, true => "ONE\n"
  | k + 2, _ => s!"many: {k + 2}"
  | _, _ => "one"

instance : Add Point where
  add p q := ⟨p.x + q.x, p.y + q.y⟩

instance [Inhabited α] : Inhabited (List α) := ⟨[default]⟩

def doubled (xs : List Nat) : List Nat :=
  xs.map fun x => x * 2

def initial : Char := 'L'

def ratio : Float := 2.5e-1

theorem add_zero_right (n : Nat) : n + 0 = n := by
  simp

theorem and_swap {p q : Prop} : p ∧ q → q ∧ p := by
  intro h
  exact ⟨h.right, h.left⟩

example : ∀ n : Nat, 0 ≤ n := fun n => Nat.zero_le n

@[inline] private def twice (f : α → α) (a : α) : α := f (f a)

noncomputable def pick (h : Nonempty α) : α := Classical.choice h

theorem unfinished : 2 + 2 = 4 := by
  sorry

def greet (stdout : IO.FS.Stream) : IO Unit := do
  let mut count := 0
  let line ← (← IO.getStdin).getLine
  for name in line.splitOn " " do
    count := count + 1
    stdout.putStrLn s!"Hello, {name}!"
  if count > 1 then
    IO.println "That's a crowd."
  else
    pure ()

#eval describe 3
#check Point.mk 1 2
#print Expr

end Tour

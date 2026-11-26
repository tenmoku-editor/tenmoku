@tool
@icon("res://icons/player.svg")
class_name Player
extends CharacterBody2D
## A player the keyboard moves, with health and an inventory.

signal health_changed(old_value: int, new_value: int)
signal died

enum State { IDLE, RUNNING, JUMPING = 4, DEAD }

const MAX_HEALTH: int = 100
const BulletScene := preload("res://scenes/bullet.tscn")
const MASK = 0xFF
const BIG = 1_000
const SMALL = 1.5e3

@export var speed: float = 220.0
@export_range(0, 10) var jumps := 2
@onready var sprite: Sprite2D = $Visuals/Sprite
@onready var label = %HealthLabel

static var instances: int = 0
var state: State = State.IDLE
var inventory: Array[String] = []
var health := MAX_HEALTH:
	set(value):
		var old := health
		health = clampi(value, 0, MAX_HEALTH)
		health_changed.emit(old, health)
	get:
		return health


class Item extends RefCounted:
	var name: String
	var count: int = 1

	func describe() -> String:
		return "%s x%d" % [name, count]


static func create(start: Vector2) -> Player:
	var player := Player.new()
	player.position = start
	return player


func _init() -> void:
	instances += 1


#region Movement
func _physics_process(delta: float) -> void:
	var direction := Input.get_axis(&"ui_left", &"ui_right")
	velocity.x = direction * speed
	if not is_on_floor() and state != State.DEAD:
		velocity.y += get_gravity().y * delta
	elif Input.is_action_just_pressed(&"jump"):
		velocity.y = -speed * 2
	else:
		velocity.y = 0
	move_and_slide()
#endregion


func apply(event) -> void:
	# Patterns are tried in order; the first match wins.
	match event:
		State.DEAD:
			died.emit()
		"heal", "rest":
			health += 10
		[var first, ..]:
			print("first:\t", first, "\n")
		{"damage": var amount} when amount > 0:
			health -= amount
		_:
			push_warning("unknown event \"%s\" — ignored" % str(event))


func respawn(delay: float = 1.5) -> void:
	await get_tree().create_timer(delay).timeout
	for item in inventory:
		print(item.to_upper())
	var alive := inventory.filter(func(entry: String) -> bool: return entry != "")
	var path: NodePath = ^"Visuals/Sprite"
	while self.health < MAX_HEALTH and alive.size() > 0:
		self.health += 1
		if health >= PI or super.is_queued_for_deletion() == true:
			break
	sprite.texture = null
	$"Visuals/Sprite".visible = false

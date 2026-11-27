--!strict
-- A small bank account module, as a Roblox ModuleScript might have it.

--[[
	Accounts keep a balance and a history of what changed it.
]]

--- The kinds of change, and a generic result.
export type Kind = "deposit" | "withdraw"
type Result<T, E = string> = { ok: true, value: T } | { ok: false, error: E }

type Entry = {
	kind: Kind,
	amount: number,
	note: string?,
	read tags: { string },
	[string]: any,
}

type Listener = (entry: Entry, index: number) -> ()

local LIMIT = 1_000
local MASK, FLAGS = 0xFF, 0b1010
local RATE: number = 2.5e-2

local Account = {}
Account.__index = Account

@native
local function clamp(x: number): number
	return math.max(0, math.min(x, math.huge))
end

local function first<T>(items: { T }, ...: T): T?
	return items[1]
end

function Account.new(owner: string, balance: number?)
	local self = setmetatable({
		owner = owner,
		balance = balance or 0,
		history = {} :: { Entry },
		frozen = false,
		parent = nil,
	}, Account)
	return self
end

function Account:deposit(amount: number): Result<number>
	if amount <= 0 then
		return { ok = false, error = "not positive" }
	elseif amount > LIMIT then
		return { ok = false, error = `over the limit of {LIMIT}` }
	else
		self.balance += clamp(amount)
	end

	table.insert(self.history, { kind = "deposit", amount = amount, tags = {} })
	return { ok = true, value = self.balance }
end

function Account:describe(): string
	local parts: { string } = {}
	for index, entry in self.history do
		if entry.amount == 0 then
			continue
		end
		parts[#parts + 1] = string.format("%d: %s\t%d", index, entry.kind, entry.amount)
	end

	for i = #parts, 1, -1 do
		print(parts[i])
	end

	local tries = 0
	while tries < 3 do
		tries += 1
	end

	repeat
		tries -= 1
	until tries == 0

	local status = if self.frozen then "frozen" else "open"
	return `{self.owner} ({status}): {table.concat(parts, ", ")}\n`
end

local account = Account.new("Ada")
local result = account:deposit(250 // 2)
if not result.ok and typeof(result) == "table" then
	warn(result.error)
end
print(account:describe(), first({ MASK, FLAGS }), RATE)

return Account

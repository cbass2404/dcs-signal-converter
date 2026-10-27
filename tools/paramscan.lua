-- Log every cockpit param that changes, to find the one that says which
-- seat the player is in, for a module DCS-BIOS has no SEAT_POSITION for.
--
-- Add to Saved Games/DCS/Scripts/Export.lua, last line, and comment it out
-- when done:
--
--   pcall(dofile, [[C:\path\to\dcs-signal-converter\tools\paramscan.lua]])
--
-- Start a mission, sit hands off for a few seconds, then change seats a few
-- times, waiting a couple of seconds in each. The log is Saved
-- Games/DCS/Logs/paramscan.log, rewritten each mission: every param as it
-- stood at the start, then model time, param, old value, new value, one line
-- per change. A param that changes more than MUTE_AFTER times is muted after
-- one line saying so, since timers and gauges change all the time; the seat
-- param is the one that steps once per seat change and is quiet otherwise.
--
-- It only reads list_cockpit_params, the same call DCS-BIOS's
-- defineSeatPosition reads SEAT_POSITION from, so a param found here is one
-- DCS-BIOS can export with that one line.

local lfs = require("lfs")
local out = io.open(lfs.writedir() .. "Logs/paramscan.log", "w")
local POLL = 0.25
local MUTE_AFTER = 20
local last = nil
local changes = {}
local next_poll = 0
local prev_after = LuaExportAfterNextFrame

-- KEY:value per line. A key can hold colons of its own, as in
-- ExternalFM:HumanInfo:FinMx:0.000000, so the value is after the last one.
local function params()
	local ret = {}
	local text = list_cockpit_params and list_cockpit_params() or ""
	for line in string.gmatch(text, "([^\n]+)") do
		local key, value = line:match("^(.*):([^:]*)$")
		if key then
			ret[key] = value
		end
	end
	return ret
end

LuaExportAfterNextFrame = function()
	if prev_after then
		pcall(prev_after)
	end
	pcall(function()
		if not out then
			return
		end
		local t = LoGetModelTime() or 0
		if t < next_poll then
			return
		end
		next_poll = t + POLL
		local now = params()
		if not last then
			local self = LoGetSelfData()
			out:write(string.format("%.2f\tSTART\t%s\n", t, self and self.Name or "?"))
			local keys = {}
			for k in pairs(now) do
				keys[#keys + 1] = k
			end
			table.sort(keys)
			for _, k in ipairs(keys) do
				out:write(string.format("\t%s\t%s\n", k, now[k]))
			end
			out:write(string.format("%.2f\tCHANGES\n", t))
		else
			for k, v in pairs(now) do
				local l = last[k]
				if v ~= l then
					local n = (changes[k] or 0) + 1
					changes[k] = n
					if n <= MUTE_AFTER then
						out:write(string.format("%.2f\t%s\t%s\t%s\n", t, k, tostring(l), v))
					elseif n == MUTE_AFTER + 1 then
						out:write(string.format("%.2f\t%s\tmuted, changes all the time\n", t, k))
					end
				end
			end
			for k, l in pairs(last) do
				if now[k] == nil then
					out:write(string.format("%.2f\t%s\t%s\tgone\n", t, k, l))
				end
			end
		end
		last = now
		out:flush()
	end)
end

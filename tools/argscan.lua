-- Log every cockpit draw arg that moves, to find the arg behind a control
-- DCS-BIOS reads wrong or does not export.
--
-- Add to Saved Games/DCS/Scripts/Export.lua, last line, and comment it out
-- when done:
--
--   pcall(dofile, [[C:\path\to\dcs-signal-converter\tools\argscan.lua]])
--
-- Start a mission, sit hands off for a few seconds, then work the control
-- slowly a few times. The log is Saved Games/DCS/Logs/argscan.log, rewritten
-- each mission: model time, arg, old value, new value, one line per move of
-- 0.02 or more. Needles and clocks move all the time; the control's arg is
-- the one that steps once per throw and is quiet otherwise.
--
-- It only reads GetDevice(0):get_argument_value, the same call DCS-BIOS
-- exports from, so an arg found here is one DCS-BIOS can read.

local lfs = require("lfs")
local out = io.open(lfs.writedir() .. "Logs/argscan.log", "w")
local last = {}
local started = false
local prev_after = LuaExportAfterNextFrame

LuaExportAfterNextFrame = function()
	if prev_after then
		pcall(prev_after)
	end
	pcall(function()
		local dev = GetDevice(0)
		if not dev or not out then
			return
		end
		local t = LoGetModelTime() or 0
		for i = 0, 999 do
			local v = dev:get_argument_value(i)
			if v then
				local l = last[i]
				if l == nil then
					last[i] = v
				elseif math.abs(v - l) >= 0.02 then
					if started then
						out:write(string.format("%.2f\t%d\t%.3f\t%.3f\n", t, i, l, v))
					end
					last[i] = v
				end
			end
		end
		if not started then
			started = true
			local self = LoGetSelfData()
			out:write(string.format("%.2f\tSTART\t%s\n", t, self and self.Name or "?"))
		end
		out:flush()
	end)
end

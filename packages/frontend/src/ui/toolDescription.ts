// ---------------------------------------------------------------------------
// ui/toolDescription.ts
//
// Produce a concise, human-readable summary for a tool call given its name
// and JSON arguments string.  Falls back to just the tool name when the
// arguments can't be parsed or the tool is unknown.
// ---------------------------------------------------------------------------

/** Max length for truncated values in the description. */
const MAX_DESC_PART = 60;

/** Truncate a string to `max` chars, appending "…" if truncated. */
function trunc(s: string, max: number = MAX_DESC_PART): string {
  if (s.length <= max) return s;
  return s.slice(0, max) + "…";
}

/**
 * Parse a JSON arguments string and return a brief human-readable summary
 * for the given tool name. Falls back to just the tool name if parsing fails
 * or the tool is unknown.
 */
export function toolDescription(name: string, argumentsJson?: string): string {
  if (!argumentsJson) return name;

  let args: Record<string, unknown>;
  try {
    args = JSON.parse(argumentsJson);
  } catch {
    return name;
  }

  switch (name) {
    case "read": {
      const filePath = String(args.file_path ?? "");
      const offset = args.offset as number | undefined;
      const limit = args.limit as number | undefined;
      if (offset || limit) {
        return `read ${filePath}:L${offset ?? 1}-${(offset ?? 1) + (limit ?? 2000) - 1}`;
      }
      return `read ${filePath}`;
    }
    case "edit": {
      const filePath = String(args.file_path ?? "");
      return `edit ${filePath}`;
    }
    case "write": {
      const filePath = String(args.file_path ?? "");
      return `write ${filePath}`;
    }
    case "grep": {
      const pattern = String(args.pattern ?? "");
      const glob = args.glob as string | undefined;
      const path = args.path as string | undefined;
      let desc = `grep "${trunc(pattern, 30)}"`;
      if (glob) desc += ` in ${glob}`;
      else if (path) desc += ` in ${path}`;
      return desc;
    }
    case "glob": {
      const pattern = String(args.pattern ?? "");
      const path = args.path as string | undefined;
      let desc = `glob ${trunc(pattern)}`;
      if (path) desc += ` in ${path}`;
      return desc;
    }
    case "bash": {
      const command = String(args.command ?? "");
      return `bash ${trunc(command)}`;
    }
    case "addition": {
      const a = args.a as number | undefined;
      const b = args.b as number | undefined;
      return `addition ${a ?? "?"} + ${b ?? "?"}`;
    }
    case "start_subagent": {
      const task = String(args.task ?? "");
      return `subagent "${trunc(task, 50)}"`;
    }
    default:
      return name;
  }
}

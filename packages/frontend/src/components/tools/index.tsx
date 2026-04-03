// ---------------------------------------------------------------------------
// components/tools/index.tsx — Tool call renderer registry
//
// Maps tool names to custom UI components. Falls back to DefaultToolCall
// for any tool without a specific renderer. To add a custom UI for a tool:
//   1. Create a component in this directory (e.g. bash-tool.tsx)
//   2. Add it to the TOOL_RENDERERS map below
// ---------------------------------------------------------------------------

import React from "react";
import { DefaultToolCall, type ToolCallProps } from "./tool-call.tsx";

type ToolRenderer = React.FC<ToolCallProps>;

/** Map of tool name → custom renderer. Add entries here for custom UIs. */
const TOOL_RENDERERS: Record<string, ToolRenderer> = {
  // Example:
  // "bash": BashToolCall,
  // "read": ReadToolCall,
};

interface Props {
  toolName: string;
  toolArgs?: string;
  content: string;
}

export function ToolCallDisplay({ toolName, toolArgs, content }: Props) {
  const Renderer = TOOL_RENDERERS[toolName] ?? DefaultToolCall;
  return <Renderer toolName={toolName} toolArgs={toolArgs} content={content} />;
}

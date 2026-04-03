// ---------------------------------------------------------------------------
// components/tools/tool-call.tsx — Default tool call display
//
// Renders a tool invocation with its name, arguments, and result.
// This is the fallback renderer — specific tools can get custom UIs
// by adding a component to this directory and registering it in the map.
// ---------------------------------------------------------------------------

import React from "react";
import { Box, Text } from "ink";

export interface ToolCallProps {
  toolName: string;
  toolArgs?: string;
  content: string;
}

export function DefaultToolCall({ toolName, toolArgs, content }: ToolCallProps) {
  return (
    <>
      <Box gap={1}>
        <Text bold color="yellow">
          ⚙ {toolName}
        </Text>
        {toolArgs && <Text dimColor>({toolArgs})</Text>}
      </Box>
      <Text dimColor>→ {content}</Text>
    </>
  );
}

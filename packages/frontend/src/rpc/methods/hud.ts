export interface CurrentGitBranchGetParams {}

export interface CurrentGitBranchGetResult {
  branch: string;
}

export interface DiffCountsGetParams {}

export interface DiffCountsGetResult {
  added: number;
  removed: number;
}

export interface ContextTokensGetParams {}

export interface ContextTokensGetResult {
  tokens: number;
}
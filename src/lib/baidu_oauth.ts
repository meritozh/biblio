export interface ParsedToken {
  access_token: string;
  expires_in_secs: number;
}

const DEFAULT_EXPIRES_IN = 2_592_000;

export function parseTokenInput(input: string): ParsedToken | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  const fragmentMatch = trimmed.match(/[#&]access_token=([^&#]+)/);
  if (fragmentMatch && fragmentMatch[1]) {
    const access_token = fragmentMatch[1];
    let expires_in_secs = DEFAULT_EXPIRES_IN;
    const expiresMatch = trimmed.match(/[#&]expires_in=(\d+)/);
    if (expiresMatch && expiresMatch[1]) {
      expires_in_secs = parseInt(expiresMatch[1], 10);
    }
    return { access_token, expires_in_secs };
  }

  return { access_token: trimmed, expires_in_secs: DEFAULT_EXPIRES_IN };
}

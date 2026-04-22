import { useEffect, useState } from 'react';
import { AlertCircle, Check, Eye, EyeOff, Loader2, LogOut } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';
import { remoteConfigGet, remoteLogin, remoteLogout } from '@/lib/tauri';
import type { RemoteAuthMode, RemoteConfig } from '@/types';

const DEFAULT_APP_ROOT = '/apps/biblio';

export function RemoteStorageSetting() {
  const [config, setConfig] = useState<RemoteConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const [authMode, setAuthMode] = useState<RemoteAuthMode>('openlist_proxy');
  const [refreshToken, setRefreshToken] = useState('');
  const [clientId, setClientId] = useState('');
  const [clientSecret, setClientSecret] = useState('');
  const [appRoot, setAppRoot] = useState(DEFAULT_APP_ROOT);
  const [showToken, setShowToken] = useState(false);

  useEffect(() => {
    remoteConfigGet()
      .then((cfg) => {
        setConfig(cfg);
        setAuthMode(cfg.auth_mode as RemoteAuthMode);
        setClientId(cfg.client_id ?? '');
        setClientSecret(cfg.client_secret ?? '');
        setAppRoot(cfg.app_root || DEFAULT_APP_ROOT);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  const handleLogin = async () => {
    setBusy(true);
    setError(null);
    setSuccess(null);
    try {
      const updated = await remoteLogin({
        auth_mode: authMode,
        refresh_token: refreshToken.trim(),
        client_id: authMode === 'self_app' ? clientId.trim() : null,
        client_secret: authMode === 'self_app' ? clientSecret.trim() : null,
        app_root: appRoot.trim() || null,
      });
      setConfig(updated);
      setRefreshToken('');
      setSuccess('Connected — access token stored.');
      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleLogout = async () => {
    setBusy(true);
    setError(null);
    try {
      await remoteLogout();
      const fresh = await remoteConfigGet();
      setConfig(fresh);
      setSuccess('Signed out of Baidu Pan.');
      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  if (loading) {
    return <div className="text-sm text-muted-foreground min-h-[100px]">Loading...</div>;
  }

  const isConnected = config?.enabled ?? false;
  const expiresInSecs = config ? config.access_token_expires_at - Math.floor(Date.now() / 1000) : 0;
  const expiresHumanReadable =
    expiresInSecs > 0 ? `${Math.floor(expiresInSecs / 60)} min` : 'expired';

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-semibold">Remote Storage (Baidu Pan)</h3>
        <p className="text-xs text-muted-foreground">
          Upload comic archives to Baidu Netdisk. Biblio keeps metadata + cover locally;
          the archive file lives on Baidu under an obfuscated filename.
        </p>
      </div>

      {isConnected && config ? (
        <div className="space-y-3 rounded-xl border border-success/30 bg-success-muted dark:bg-success/10 p-3">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Badge variant="green">
                <Check className="h-3 w-3 mr-1" /> Connected
              </Badge>
              <span className="text-xs text-muted-foreground">
                {config.auth_mode === 'openlist_proxy' ? 'via OpenList proxy' : 'self-hosted app'}
              </span>
            </div>
            <Button variant="outline" size="sm" onClick={handleLogout} disabled={busy}>
              <LogOut className="h-3 w-3 mr-1" /> Sign out
            </Button>
          </div>
          <div className="text-xs text-muted-foreground space-y-0.5">
            <div>Upload root: <span className="font-mono">{config.app_root}</span></div>
            <div>Access token: {expiresHumanReadable}</div>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          <div className="space-y-1.5">
            <Label className="text-xs">Authentication</Label>
            <div className="grid grid-cols-1 gap-2">
              <AuthModeOption
                label="OpenList proxy (recommended)"
                description="OpenList holds the Baidu AppKey; you only paste the refresh token from their authorize URL."
                selected={authMode === 'openlist_proxy'}
                onSelect={() => setAuthMode('openlist_proxy')}
              />
              <AuthModeOption
                label="Self-hosted Baidu app"
                description="Use your own AppKey + Secret registered at pan.baidu.com/union/console."
                selected={authMode === 'self_app'}
                onSelect={() => setAuthMode('self_app')}
              />
            </div>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="baidu-refresh-token" className="text-xs">
              Refresh Token
            </Label>
            <div className="relative">
              <Input
                id="baidu-refresh-token"
                type={showToken ? 'text' : 'password'}
                value={refreshToken}
                onChange={(e) => setRefreshToken(e.target.value)}
                placeholder="Paste the refresh_token from your Baidu authorize flow"
                className="text-sm pr-10"
              />
              <button
                type="button"
                onClick={() => setShowToken(!showToken)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                aria-label={showToken ? 'Hide token' : 'Show token'}
              >
                {showToken ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
          </div>

          {authMode === 'self_app' && (
            <>
              <div className="space-y-1.5">
                <Label htmlFor="baidu-client-id" className="text-xs">
                  Client ID (AppKey)
                </Label>
                <Input
                  id="baidu-client-id"
                  value={clientId}
                  onChange={(e) => setClientId(e.target.value)}
                  className="text-sm"
                />
              </div>
              <div className="space-y-1.5">
                <Label htmlFor="baidu-client-secret" className="text-xs">
                  Client Secret
                </Label>
                <Input
                  id="baidu-client-secret"
                  type="password"
                  value={clientSecret}
                  onChange={(e) => setClientSecret(e.target.value)}
                  className="text-sm"
                />
              </div>
            </>
          )}

          <div className="space-y-1.5">
            <Label htmlFor="baidu-app-root" className="text-xs">
              Upload directory
            </Label>
            <Input
              id="baidu-app-root"
              value={appRoot}
              onChange={(e) => setAppRoot(e.target.value)}
              placeholder="/apps/biblio"
              className="text-sm font-mono"
            />
            <p className="text-xs text-muted-foreground">
              Absolute path under your Baidu Pan. Must live inside an `/apps/…` directory
              when using the Baidu OpenAPI; biblio creates it implicitly on first upload.
            </p>
          </div>

          <Button size="sm" onClick={handleLogin} disabled={busy || !refreshToken.trim()}>
            {busy ? <Loader2 className="h-4 w-4 animate-spin mr-1" /> : null}
            Connect
          </Button>
        </div>
      )}

      {error && (
        <div className="flex items-center gap-2 text-xs text-destructive">
          <AlertCircle className="h-3 w-3" />
          {error}
        </div>
      )}
      {success && (
        <div className="flex items-center gap-2 text-xs text-success">
          <Check className="h-3 w-3" />
          {success}
        </div>
      )}
    </div>
  );
}

function AuthModeOption({
  label,
  description,
  selected,
  onSelect,
}: {
  label: string;
  description: string;
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        'flex items-start gap-2 p-2 rounded-lg border text-left transition-colors duration-200',
        selected ? 'border-primary bg-primary/5' : 'hover:bg-secondary'
      )}
    >
      <span
        className={cn(
          'mt-1 h-3 w-3 rounded-full border shrink-0',
          selected ? 'border-primary bg-primary' : 'border-muted-foreground/40'
        )}
      />
      <span>
        <span className="block text-sm font-medium">{label}</span>
        <span className="block text-xs text-muted-foreground">{description}</span>
      </span>
    </button>
  );
}

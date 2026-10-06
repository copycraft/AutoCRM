'use client';

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import {
  LayoutDashboard,
  Building2,
  Target,
  Package,
  Receipt,
  Mail,
  BarChart3,
  MonitorPlay,
  Settings,
  ShieldCheck,
  LogOut,
  Snowflake,
  Users,
  Bell,
  SlidersHorizontal,
  Megaphone,
  Inbox,
  FileText,
} from 'lucide-react';
import { cn } from '@/lib/utils/format';
import { useAuth, canAdmin, canAccessHr } from '@/lib/auth/context';
import { GlobalSearch } from '@/components/search/GlobalSearch';
import { useNotifications } from '@/components/notifications/useUnread';

const NAV: { href: string; icon: typeof LayoutDashboard; key: string; admin?: boolean; hr?: boolean }[] = [
  { href: '', icon: LayoutDashboard, key: 'dashboard' },
  { href: '/notifications', icon: Bell, key: 'notifications' },
  { href: '/partners/business', icon: Building2, key: 'business' },
  { href: '/leads', icon: Target, key: 'leads' },
  { href: '/marketing', icon: Megaphone, key: 'marketing' },
  { href: '/orders', icon: Package, key: 'orders' },
  { href: '/invoices', icon: Receipt, key: 'invoices' },
  { href: '/incoming-invoices', icon: Inbox, key: 'incoming' },
  { href: '/emails', icon: Mail, key: 'emails' },
  { href: '/templates', icon: FileText, key: 'templates' },
  { href: '/board', icon: MonitorPlay, key: 'board' },
  { href: '/reports', icon: BarChart3, key: 'reports' },
  { href: '/hr', icon: Users, key: 'hr', hr: true },
  { href: '/admin', icon: ShieldCheck, key: 'admin', admin: true },
  { href: '/settings', icon: Settings, key: 'settings', admin: true },
] as const;

export function Sidebar({ onHelp }: { onHelp?: () => void }) {
  const t = useTranslations('navigation');
  const tq = useTranslations('qol');
  const tc = useTranslations('common');
  const tn = useTranslations('notifications');
  const pathname = usePathname();
  const locale = useLocale();
  const { user, logout } = useAuth();
  const unread = useNotifications().data?.unread ?? 0;

  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-steel-200 bg-surface">
      <Link href={`/${locale}`} className="flex items-center gap-3 px-5 py-4 border-b border-steel-200">
        <span className="flex h-9 w-9 items-center justify-center rounded-lg bg-steel-900 text-surface">
          <Snowflake className="h-5 w-5" aria-hidden />
        </span>
        <span className="leading-tight">
          <span className="block text-body font-semibold tracking-wide">AUTOTHERM</span>
          <span className="block text-metadata text-steel-500">AutoCRM</span>
        </span>
      </Link>

      <GlobalSearch />

      <nav className="flex-1 overflow-y-auto p-3 space-y-0.5" aria-label={t('main')}>
        {NAV.filter((i) => (!i.admin || canAdmin(user)) && (!i.hr || canAccessHr(user))).map((item) => {
          const href = `/${locale}${item.href}`;
          const active = pathname === href || (item.href !== '' && pathname.startsWith(href + '/'));
          const Icon = item.icon;
          return (
            <Link
              key={item.key}
              href={href}
              aria-current={active ? 'page' : undefined}
              className={cn(
                'flex items-center gap-3 rounded-lg px-3 py-2 text-body font-medium transition-colors',
                active ? 'bg-steel-900 text-surface' : 'text-steel-900 hover:bg-panel',
              )}
            >
              <Icon className="h-4 w-4 shrink-0" aria-hidden />
              {t(item.key)}
              {item.key === 'notifications' && unread > 0 && (
                <span
                  className="ml-auto rounded-full bg-signal px-2 py-0.5 text-metadata font-semibold text-surface"
                  aria-label={`${unread} ${tn('unread')}`}
                >
                  {unread > 99 ? '99+' : unread}
                </span>
              )}
            </Link>
          );
        })}
      </nav>

      <div className="border-t border-steel-200 p-3">
        <div className="px-2 pb-2">
          <p className="text-body font-medium truncate">{user?.display_name}</p>
          <p className="text-metadata text-steel-500 truncate">{user?.email}</p>
        </div>
        <Link href={`/${locale}/preferences`} className="btn-ghost btn-sm w-full justify-start">
          <SlidersHorizontal className="h-4 w-4" aria-hidden />
          {t('preferences')}
        </Link>
        <button onClick={() => void logout()} className="btn-ghost btn-sm w-full justify-start">
          <LogOut className="h-4 w-4" aria-hidden />
          {tc('logout')}
        </button>
        {onHelp && (
          <button
            onClick={onHelp}
            className="btn-ghost btn-sm w-full justify-start"
            title="?"
            aria-label="?"
          >
            <kbd className="rounded border border-steel-200 px-1.5 font-mono text-metadata text-steel-500">
              ?
            </kbd>
            {tq('shortcutsTitle')}
          </button>
        )}
      </div>
    </aside>
  );
}

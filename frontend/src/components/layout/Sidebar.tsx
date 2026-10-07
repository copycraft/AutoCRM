'use client';

import { useEffect, useState } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { useLocale, useTranslations } from 'next-intl';
import {
  LayoutDashboard,
  Building2,
  Package,
  Receipt,
  Mail,
  BarChart3,

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
  ParkingSquare,
  ShieldAlert,
  ChevronDown,
  Shield,
} from 'lucide-react';
import { cn } from '@/lib/utils/format';
import { useAuth, canAdmin, canAccessHr } from '@/lib/auth/context';
import { GlobalSearch } from '@/components/search/GlobalSearch';
import { useNotifications } from '@/components/notifications/useUnread';
import { isMarketingPath, marketingHref } from '@/components/layout/MarketingTabs';

const NAV: { href: string; icon: typeof LayoutDashboard; key: string; admin?: boolean; hr?: boolean }[] = [
  { href: '', icon: LayoutDashboard, key: 'dashboard' },
  { href: '/notifications', icon: Bell, key: 'notifications' },
  { href: '/partners/business', icon: Building2, key: 'business' },
  // Leads live under Marketing, as its second tab (MarketingTabs).
  { href: '/marketing', icon: Megaphone, key: 'marketing' },
  { href: '/orders', icon: Package, key: 'orders' },
  { href: '/invoices', icon: Receipt, key: 'invoices' },
  { href: '/incoming-invoices', icon: Inbox, key: 'incoming' },
  { href: '/emails', icon: Mail, key: 'emails' },
  { href: '/templates', icon: FileText, key: 'templates' },

  { href: '/yard', icon: ParkingSquare, key: 'yard' },
  { href: '/incidents', icon: ShieldAlert, key: 'incidents' },
  { href: '/reports', icon: BarChart3, key: 'reports' },
  { href: '/hr', icon: Users, key: 'hr', hr: true },
  { href: '/admin', icon: ShieldCheck, key: 'admin', admin: true },
  { href: '/settings', icon: Settings, key: 'settings', admin: true },
  { href: '/privacy', icon: Shield, key: 'privacy' },
] as const;

const SECTIONS: { label: string; items: string[] }[] = [
  { label: 'Fő', items: ['dashboard', 'notifications', 'reports'] },
  { label: 'Üzlet', items: ['business', 'orders', 'invoices', 'incoming'] },
  { label: 'Művelet', items: ['yard', 'incidents'] },
  { label: 'Rendszer', items: ['hr', 'admin', 'settings', 'privacy'] },
];

function CollapsibleSection({
  label,
  defaultOpen,
  children,
}: {
  label: string;
  defaultOpen: boolean;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  useEffect(() => setOpen(defaultOpen), [defaultOpen]);
  return (
    <div>
      <button
        onClick={() => setOpen(!open)}
        className="flex w-full items-center gap-2 rounded-lg px-3 py-1.5 text-metadata font-semibold uppercase tracking-wider hover:bg-panel transition-colors"
        aria-expanded={open}
      >
        <ChevronDown className={cn('h-3.5 w-3.5 transition-transform', !open && '-rotate-90')} aria-hidden />
        {label}
      </button>
      {open && <div className="space-y-0.5 mt-0.5">{children}</div>}
    </div>
  );
}

export function Sidebar({ onHelp }: { onHelp?: () => void }) {
  const t = useTranslations('navigation');
  const tq = useTranslations('qol');
  const tc = useTranslations('common');
  const tn = useTranslations('notifications');
  const pathname = usePathname();
  const locale = useLocale();
  const { user, logout } = useAuth();
  const unread = useNotifications().data?.unread ?? 0;
  // Marketing opens the tab used last (newsletter or leads); read after mount, so the
  // first paint and the server agree.
  const [marketing, setMarketing] = useState(`/${locale}/marketing`);
  useEffect(() => setMarketing(marketingHref(locale)), [locale, pathname]);

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

      <nav className="flex-1 overflow-y-auto p-3 space-y-1" aria-label={t('main')}>
        {(() => {
          const item = NAV.find((i) => i.key === 'marketing')!;
          const href = marketing;
          const active = isMarketingPath(pathname, locale);
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
            </Link>
          );
        })()}
        {SECTIONS.map((section) => {
          const items = NAV.filter(
            (i) => section.items.includes(i.key) && (!i.admin || canAdmin(user)) && (!i.hr || canAccessHr(user)),
          );
          if (items.length === 0) return null;
          const alwaysOpen = section.label === 'Fő' || section.label === 'Üzlet';
          const isActive = items.some((item) => {
            const isMarketing = item.key === 'marketing';
            return isMarketing
              ? isMarketingPath(pathname, locale)
              : pathname === `/${locale}${item.href}` || (item.href !== '' && pathname.startsWith(`/${locale}${item.href}/`));
          });
          const content = items.map((item) => {
            const isMarketing = item.key === 'marketing';
            const href = isMarketing ? marketing : `/${locale}${item.href}`;
            const active = isMarketing
              ? isMarketingPath(pathname, locale)
              : pathname === href || (item.href !== '' && pathname.startsWith(href + '/'));
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
          });
          if (alwaysOpen) {
            return (
              <div key={section.label} className="space-y-0.5">
                <p className="px-3 py-1.5 text-metadata font-semibold uppercase tracking-wider">{section.label}</p>
                {content}
              </div>
            );
          }
          return (
            <CollapsibleSection key={section.label} label={section.label} defaultOpen={isActive}>
              {content}
            </CollapsibleSection>
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

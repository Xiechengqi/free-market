import type { RouteMeta } from 'vue-router';
import ElegantVueRouter from '@elegant-router/vue/vite';
import type { RouteKey } from '@elegant-router/types';

export function setupElegantRouter() {
  return ElegantVueRouter({
    layouts: {
      base: 'src/layouts/base-layout/index.vue',
      blank: 'src/layouts/blank-layout/index.vue'
    },
    routePathTransformer(routeName, routePath) {
      const key = routeName as RouteKey;

      // Custom non-default paths so the file layout (folder = route name) stays
      // simple but the URL matches what the rest of the codebase expects.
      if (key === 'order-detail') return '/orders/:id';
      if (key === 'product-cards') return '/products/:id/cards';

      return routePath;
    },
    onRouteMetaGen(routeName) {
      const key = routeName as RouteKey;

      const constantRoutes: RouteKey[] = ['login', '403', '404', '500'];

      const meta: Partial<RouteMeta> = {
        title: key,
        i18nKey: `route.${key}` as App.I18n.I18nKey
      };

      if (constantRoutes.includes(key)) {
        meta.constant = true;
      }

      // Hide detail routes from the sidebar and keep parent menu highlighted.
      if (key === 'order-detail') {
        meta.hideInMenu = true;
        meta.activeMenu = 'orders';
      }
      if (key === 'product-cards') {
        meta.hideInMenu = true;
        meta.activeMenu = 'products';
      }
      // Sidebar grouping order. Falls back to elegant-router default order otherwise.
      const orderMap: Record<string, number> = {
        home: 1,
        orders: 2,
        products: 3,
        categories: 4,
        cards: 5,
        coupons: 6,
        'payment-channels': 7,
        'email-templates': 8,
        admins: 9,
        settings: 10,
        jobs: 11,
        'notification-logs': 12,
        'audit-logs': 13,
        trash: 14,
        uploads: 15,
        backup: 16
      };
      if (orderMap[key] !== undefined) {
        meta.order = orderMap[key];
      }
      // Owner-only sections.
      if (['admins', 'settings', 'audit-logs', 'backup'].includes(key)) {
        meta.roles = ['owner'];
      }

      return meta;
    }
  });
}

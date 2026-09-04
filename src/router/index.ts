import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'projects',
      component: () => import('@/views/ProjectsView.vue'),
      meta: { phaseTitle: '阶段 1 · 项目管理' },
    },
    {
      path: '/project/:id',
      name: 'project',
      component: () => import('@/views/ProjectView.vue'),
      meta: { phaseTitle: '阶段 1 · 项目工作区' },
    },
    {
      path: '/env',
      name: 'env',
      component: () => import('@/views/HomeView.vue'),
      meta: { phaseTitle: '阶段 0 · 环境自检' },
    },
  ],
})

export default router

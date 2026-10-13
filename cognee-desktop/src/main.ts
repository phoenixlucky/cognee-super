import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
import '@vue-flow/core/dist/style.css'
import '@vue-flow/core/dist/theme-default.css'
import './style.css'
import App from './App.vue'
import router from './router'
import { useAppStore } from './stores/app'

const pinia = createPinia()
createApp(App).use(pinia).use(router).use(ElementPlus).mount('#app')
void useAppStore(pinia).listenBootstrap()

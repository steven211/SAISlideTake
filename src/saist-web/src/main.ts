import { createApp } from 'vue';
import App from './App.vue';
import './styles/base.css';
import './styles/capsule.css';
import './styles/drawer.css';
import './styles/studio_matrix.css';
import './styles/settings_tab.css';
import { applyLocale } from './locales';

applyLocale();
createApp(App).mount('#app');

import { NgModule } from '@angular/core';
import { Routes, RouterModule } from '@angular/router';
import { DevicesComponent } from './devices/devices.component';
import { SettingsComponent } from './settings/settings.component';
import { LoginComponent } from './login/login.component';
import { LogComponent } from './log/log.component';

const routes: Routes = [
  { path: 'login', component: LoginComponent },
  { path: 'devices', component: DevicesComponent },
  { path: 'settings', component: SettingsComponent },
  { path: 'log', component: LogComponent },
  { path: '**', redirectTo: '/devices' }
];

@NgModule({
  imports: [RouterModule.forRoot(routes)],
  exports: [RouterModule]
})
export class AppRoutingModule { }

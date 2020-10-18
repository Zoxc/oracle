import { NgModule } from '@angular/core';
import { Routes, RouterModule } from '@angular/router';
import { DevicesComponent } from './devices/devices.component';
import { SettingsComponent } from './settings/settings.component';

const routes: Routes = [
  { path: '', pathMatch: 'full', redirectTo: '/devices' },
  { path: 'devices', component: DevicesComponent },
  { path: 'settings', component: SettingsComponent }
];

@NgModule({
  imports: [RouterModule.forRoot(routes)],
  exports: [RouterModule]
})
export class AppRoutingModule { }

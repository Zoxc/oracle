import { HttpClient } from '@angular/common/http';
import { Component, OnInit } from '@angular/core';
import { FormBuilder, FormControl, FormGroup, Validators, ValidatorFn } from '@angular/forms';

function tcp_port_validator(control: FormControl) {
  if (isNaN(Number(control.value))) {
    return { "test": "hm" };
  }

  let number = parseInt(control.value, 10);

  if (!isNaN(number) && number > 0 && number < 65536) {
    return null;
  }

  return { "test": "hm" };
}

@Component({
  selector: 'app-settings',
  templateUrl: './settings.component.html',
  styleUrls: ['./settings.component.scss']
})

export class SettingsComponent implements OnInit {
  initial: any;
  loaded = false;
  form = new FormGroup({
    "web_port": new FormControl(null, tcp_port_validator),
    "ping_interval": new FormControl(null, tcp_port_validator)
  });
  constructor(private http: HttpClient) { }

  ngOnInit(): void {
    this.http.get("/api/settings").subscribe(data => {
      this.initial = data;
      this.form.setValue(data);
      this.loaded = true;
    });
  }

  apply() {
    let data = this.form.value;
    data.web_port = parseInt(data.web_port);
    data.ping_interval = parseInt(data.ping_interval);
    this.form.reset(data);
    this.http.post("/api/settings", data).subscribe(_dummy => { })
  }
}

import { HttpClient } from '@angular/common/http';
import { Component, OnInit } from '@angular/core';
import { FormControl, FormGroup } from '@angular/forms';
import { Router } from '@angular/router';

@Component({
  selector: 'app-login',
  templateUrl: './login.component.html',
  styleUrls: ['./login.component.scss']
})
export class LoginComponent implements OnInit {
  invalid = false
  form = new FormGroup({
    "name": new FormControl(null),
    "password": new FormControl(null)
  });

  constructor(private http: HttpClient, private router: Router) { }

  ngOnInit(): void {
  }

  login() {
    let data = this.form.value;
    data.name = data.name || "";
    data.password = data.password || "";
    this.http.post("/api/login", data).subscribe(data => {
      let result = data as any;
      if (result.result == "error") {
        this.invalid = true;
      } else if (result.result == "ok") {
        this.router.navigate(['/devices']);
      }
    })
  }
}

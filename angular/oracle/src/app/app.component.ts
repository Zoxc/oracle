import { HttpClient } from '@angular/common/http';
import { Component } from '@angular/core';
import { ActivatedRoute, Router } from '@angular/router';

@Component({
  selector: 'app-root',
  templateUrl: './app.component.html',
  styleUrls: ['./app.component.scss']
})
export class AppComponent {
  url = null;
  isCollapsed = false;

  constructor(private route: ActivatedRoute, private router: Router, private http: HttpClient) {
  }

  ngOnInit() {
    console.log(this.router.url);
    this.router.url;
    this.route.url.subscribe(url => {
      console.log(url);
      this.url = url;
    });
  }

  logout() {
    this.http.post("/api/logout", {}).subscribe(_data => {
      this.router.navigate(['/login']);
    })
  }
}

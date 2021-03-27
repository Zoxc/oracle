import { Injectable } from '@angular/core';
import { HttpRequest, HttpHandler, HttpEvent, HttpInterceptor } from '@angular/common/http';

import { Observable } from 'rxjs';
import { LoginService } from './login.service';

@Injectable()
export class JwtInterceptor implements HttpInterceptor {
    constructor(private loginService: LoginService) { }

    intercept(request: HttpRequest<any>, next: HttpHandler): Observable<HttpEvent<any>> {
        if (this.loginService.token) {
            request = request.clone({
                setHeaders: {
                    Authorization: `Bearer ${this.loginService.token}`
                }
            });
        }

        return next.handle(request);
    }
}